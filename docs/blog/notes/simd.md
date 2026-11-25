# SIMD design article notes

*Designing the fastest general-purpose SIMD CSV parser*

- Historical outline wrt simd-json etc.
- Why hasn't it happened already? (IO-bound, streaming, branching not invalidated, very good existing scalar designs)
- copy must not happen per cell but in larger swathes, see new byterecord design. check full design notes
- Shuffling, PCMESTRI, LUTs and PCLMULQDQ seems globally bad in the case of CSV. simd-json searches for many more characters than what is needed in CSV
- You cannot expect x16 or x32, that is just not realistic.
- Branching is data-dependent and cannot be avoided.
- Complete branchlessness requires too much SIMD work with is not amortized vs. scalar in the case of CSV (the same cannot be said of JSON). JSON is way more expensive to parse.
- Zero copy is not very convenient and takes a performance hit as soon as we must unescape more than a single cell. Else we need too much bookkeeping and we could as well do copy parsing.
- Typical `memchr` SIMD string searches. The necessity to amortize the mask for dense sections. But also the structure of CSV makes it useful to have a "two-speed" approach wrt quoting.
- The desing needs to remain practical for a wide range of CSV data and not favor one specific case (it is easy to have a SIMD parser out-perform on files having very large records & cells).
- Discussing the three-bytes Searcher vs. two-bytes/more branching on delimiter version.
- Show the bench table for reference.
- Keeping a field start state is complex and can kill performance by introducing too much branching

Things tried:

Some section listing what was tried and what did not ultimately improve performance.

- Pooling the SIMD operations (Sep's approach) and running the state machine over collected structural characters. This means you can have aligned/cache buffers for the SIMD calls etc. and they are not throttled by the scalar side. It does not seem to out-perform the interleaved/inline operation. It seems to be problematic for stack frames/caches etc. to go beyond single registers for masking.
- Doing all the parse inside the amortized mask section under-peforms vs. the two-speed switch. Which means branching the state machine once per structural char.
- Alternative method with one quote+newline movemask and one delimiter movemask to deal with delimiters branchlessly which is cleared by the first quote or newline. This under-performs because of the additional movemask, and branchlessness over little chunks of data still pays the cost of the loop. Generally, prefix-sum-based methods such as pclmulqdq etc. are not worth it. What you gain in branchlessness, you lose in uops wrt the number of SIMD instructions you have to perform to make it up for it. CSV is not that complex.
- AVX2 underperforms (in the unquote sections), vzeroupper makes code generation tricky (I tried complex non-inlined version to avoid this). Unrolled SSE2 is on par for cases where AVX2 shines and is better for other cases, and is better for compatibility.
- Two-bytes searching (delimiter + newline) only, with a state for field start: more robust, less performant and result in a splitter that does not agree with the others.
- 64 bit masks seem to be the sweet spot on modern machines. 32 bits is a bit low. There is always, when branching, a tradeoff between vector width and the amount of work you will lose if you hit a quote or newline. Rolling masks across buffer or record boundary is not worth it if you keep masks in registers. The branching version also has the benefit of being able to switch gears sometimes, but for this you need to be able to throw some work into the bin.

Regarding CSV Writer:

- SIMD to find if a cell must be quoted is generally bad, because of the distribution of cell lengths and first byte to quote. The unrolled table test from burntsushi is the best.
- SIMD is useful to perform the quoting again because of the same intuition behind two-speed.
