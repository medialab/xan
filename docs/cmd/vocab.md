<!-- Generated -->
# xan vocab

```txt
Compute vocabulary statistics over tokenized documents (typically produced
by the "xan tokenize words" subcommand), i.e. rows of CSV data containing
a "tokens" column containing word tokens separated by a single space (or
any separator given to the --sep flag).

The command considers, by default, documents to be a single row of the input
but can also be symbolized by the value of a column selection given to -D/--doc.

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
    - idf: logarithm of the inverse document frequency of the token
    - gfidf: global frequency * idf for the token
    - pigeon: ratio between df and expected df in random distribution

3. doc-level statistics (using the "doc" subcommand):
    - (*doc): columns representing the document (named like the input)
    - token_count: total number of tokens in document
    - distinct_token_count: number of distinct tokens in document

4. doc-token-level statistics (using the "doc-token" subcommand):
    - (*doc): columns representing the document (named like the input)
    - token: some distinct document token (the column will be named like the input)
    - tf: term frequency for the token in the document
    - expected_tf: expected absolute term frequency (does not follow --tf-weight)
    - tfidf: term frequency * idf for the token in the document
    - bm25: BM25 score for the token in the document
    - chi2: chi2 score for the token in the document

5. token-cooccurrence-level statistics (using the "cooc" subcommand):
    - token1: the first token
    - token2: the second token
    - count: number of co-occurrences
    - expected_count: expected number of co-occurrences
    - chi2: chi2 score (approx. without the --complete flag)
    - G2: G2 score (approx. without the --complete flag)
    - pmi: pointwise mutual information
    - npmi: normalized pointwise mutual information

    or, using the --distrib flag:

    - token1: the first token
    - token2: the second token
    - count: number of co-occurrences
    - expected_count: expected number of co-occurrences
    - sd_I: distributional score based on PMI
    - sd_G2: distributional score based on G2

    or, using the --specificity flag (NOT CORRECT YET! DO NOT USE!):

    - token: the token
    - count: total number of co-occurrences
    - lgl: the specificity score (ratio of statistically relevant co-occurrences)

Note that you should generally avoid giving too much importance wrt
the statistical relevance of both chi2 & G2 scores when considering
less than 5 items (absolute term frequencies or co-occurrence counts).

Usage:
    xan vocab corpus [options] [<input>]
    xan vocab token [options] [<input>]
    xan vocab doc [options] [<input>]
    xan vocab doc-token [options] [<input>]
    xan vocab cooc [options] [<input>]
    xan vocab --help

vocab options:
    -T, --token <token-col>  Name of column containing the tokens. Will default
                             to "tokens" or "token" if --implode is given.
    -D, --doc <doc-cols>     Optional selection of columns representing a row's document.
                             Each row of input will be considered as its own document if
                             the flag is not given.
    --sep <delim>            Delimiter used to separate tokens in one row's token cell.
                             Will default to a single space.
    --implode                If given, will implode the file over the token column so that
                             it becomes possible to process a file containing only one token
                             per row. Cannot be used without -D, --doc.

vocab doc-token options:
    --tf-weight <weight>         TF weighting scheme. One of "count", "binary", "ratio",
                                 or "log-normal". [default: count]
    --k1-value <value>           "k1" Factor for BM25 computation. [default: 1.2]
    --b-value <value>            "b"  Factor for BM25 computation. [default: 0.75]
    --chi2-significance <value>  Filter doc,token pairs by only keeping significant ones wrt their
                                 chi2 score that must be above the given significance level. Accepted
                                 levels include "0.5", "0.1", "0.05", "0.025", "0.01",
                                 "0.005" and "0.001".

vocab cooc options:
    -w, --window <n>             Size of the co-occurrence window, in number of tokens around the currently
                                 considered token. If not given, co-occurrences will be computed using the bag
                                 of words model where tokens are considered to co-occur with every
                                 other one in the same document.
                                 Set the window to "1" to compute bigram collocations. Set a larger window
                                 to get something similar to what word2vec would consider.
    -F, --forward                Whether to only consider a forward window when traversing token contexts.
    --distrib                    Compute directed distributional similarity metrics instead.
    --specificity                Compute the lgl specificity score per token instead.
    --min-count <n>              Minimum number of co-occurrence count to be included in the result.
                                 [default: 1]
    --chi2-significance <value>  Filter doc,token pairs by only keeping significant ones wrt their
                                 chi2 score that must be above the given significance level. Accepted
                                 levels include "0.5", "0.1", "0.05", "0.025", "0.01",
                                 "0.005" and "0.001".
    --G2-significance <value>    Filter doc,token pairs by only keeping significant ones wrt their
                                 G2 score that must be above the given significance level. Accepted
                                 levels include "0.5", "0.1", "0.05", "0.025", "0.01",
                                 "0.005" and "0.001".

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be interpreted
                           as headers.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
