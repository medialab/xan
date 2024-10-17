<!-- Generated -->
# xan vocab

```txt
Compute vocabulary statistics over tokenized documents. Those documents
must be given as a CSV with one or more column representing a document key
and a column containing single tokens like so (typically an output from
the "tokenize" command):

doc,token
1,the
1,cat
1,eats
2,hello

This command can compute 5 kinds of differents vocabulary statistics:

1. corpus-level statistics (using the "corpus" subcommand):
    - doc_count: number of documents in the corpus
    - token_count: total number of tokens in the corpus
    - distinct_token_count: number of distinct tokens in the corpus
    - average_doc_len: average number of tokens per document

2. token-level statistics (using the "token" subcommand):
    - token: some distinct token (the column will be named like the input)
    - gf: global frequency of the token across corpus
    - df: document frequency of the token
    - df_ratio: proportion of documents containing the token
    - idf: inverse document frequency of the token
    - gfidf: global frequency * idf for the token
    - pigeonhole: ratio between df and expected df in random distribution

3. doc-level statistics (using the "doc" subcommand):
    - (*doc): columns representing the document (named like the input)
    - token_count: total number of tokens in document
    - distinct_token_count: number of distinct tokens in document

4. doc-token-level statistics (using the "doc-token" subcommand):
    - (*doc): columns representing the document (named like the input)
    - token: some distinct documnet token (the column will be named like the input)
    - tf: term frequency for the token in the document
    - tfidf: term frequency * idf for the token in the document
    - bm25: BM25 score for the token in the document
    - chi2: chi2 score for the token in the document

5. token-cooccurrence-level statistics (using the "cooc" subcommand):
    - token1: the first token
    - token2: the second token
    - count: total number of co-occurrences
    - chi2: chi2 score
    - G2: G2 score
    - pmi: pointwise mutual information
    - ppmi: positive pointwise mutual information
    - npmi: normalized pointwise mutual information

    or, using the --distrib flag:

    - token1: the first token
    - token2: the second token
    - count: total number of co-occurrences
    - sdI: distributional score based on PMI
    - sdG2: distributional score based on G2

Usage:
    xan vocab corpus <token-col> [options] [<input>]
    xan vocab token <token-col> [options] [<input>]
    xan vocab doc <token-col> [options] [<input>]
    xan vocab doc-token <token-col> [options] [<input>]
    xan vocab cooc <token-col> [options] [<input>]
    xan vocab --help

vocab options:
    -D, --doc <doc-cols>  Optional selection of columns representing a row's document.
    --sep <delim>         Delimiter used to separate tokens in one row's token cell.

vocab doc-token options:
    --k1-value <value>     "k1" factor for BM25 computation. [default: 1.2]
    --b-value <value>      "b" factor for BM25 computation. [default: 0.75]

vocab cooc options:
    -w, --window <n>  Size of the co-occurrence window. If not given, co-occurrence will be based
                      on the bag of word model where token are considered to co-occur with every
                      other one in a same document.
                      Set the window to "1" to compute bigram collocations. Set a larger window
                      to get something similar to what word2vec considers.
    -F, --forward     Whether to only consider a forward window when traversing token contexts.
    --distrib         Compute directed distributional similarity metrics instead.
    --min-count <n>   Minimum number of co-occurrence count to be included in the result.
                      [default: 1]

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will NOT be included
                           in the frequency table. Additionally, the 'field'
                           column will be 1-based indices instead of header
                           names.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
