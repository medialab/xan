<!-- Generated -->
# xan tokenize

```txt
Tokenize the given text column by splitting it into word pieces (think
words, numbers, hashtags etc.) or paragraphs (using the --paragraphs flag)
or sentences (using the --sentences) flag.

This command will therefore emit one row
per token written in a new column added at the end, all while dropping
the original text column unless --sep or --keep-text is passed.

For instance, given the following input:

id,text
1,one cat eats 2 mice! 😎
2,hello

The following command:
    $ xan tokenize text -T type file.csv

Will produce the following result:

id,token,type
1,one,word
1,cat,word
1,eats,word
1,2,number
1,mice,word
1,!,punct
1,😎,emoji
2,hello,word

You can easily pipe the command into "xan vocab" to create a vocabulary:
    $ xan tokenize text file.csv | xan vocab id token > vocab.csv

You can easily keep the tokens in a separate file using the "tee" command:
    $ xan tokenize text file.csv | tee tokens.csv | xan vocab id token > vocab.csv

This tokenizer is able to distinguish between the following types of tokens:
    - word
    - number
    - hashtag
    - mention
    - emoji
    - punct
    - url
    - email

Usage:
    xan tokenize [options] <column> [<input>]
    xan tokenize --help

tokenize options:
    -c, --column <name>      Name for the token column. Will default to "token" or "tokens"
                             if --sep is given, or "paragraph"/"paragraphs" respectively
                             if --paragraphs is given, or "sentence"/"sentences" if --sentences
                             is given.
    --paragraphs             Split paragraphs instead of words.
    --sentences              Split sentences instead of words.
    -S, --simple             Use a simpler, more performant variant of the tokenizer but unable
                             to infer token types, nor handle subtle cases.
    -N, --ngrams <n>         If given, will output token ngrams using the given n or the given
                             range of n values using a comma as separator e.g. "1,3".
                             This cannot be used with -T, --token-type.
    -T, --token-type <name>  Name of a column to add containing the type of the tokens.
                             This cannot be used with -N, --ngrams.
    -D, --drop <types>       Types of tokens to drop from the results, separated by comma,
                             e.g. "word,number". Cannot work with -k, --keep.
                             See the list of recognized types above.
    -K, --keep <types>       Types of tokens to keep in the results, separated by comma,
                             e.g. "word,number". Cannot work with -d, --drop.
                             See the list of recognized types above.
    -m, --min-token <n>      Minimum characters count of a token to be included in the output.
    -M, --max-token <n>      Maximum characters count of a token to be included in the output.
    --stoplist <path>        Path to a .txt stoplist containing one word per line.
    -J, --filter-junk        Whether to apply some heuristics to filter out words that look like junk.
    -L, --lower              Whether to normalize token case using lower case.
    -U, --unidecode          Whether to normalize token text to ascii.
    --split-hyphens          Whether to split tokens by hyphens.
    --stemmer <name>         Stemmer to normalize the tokens. Can be one of:
                                - "s": a basic stemmer removing typical plural inflections in
                                         most European languages.
                                - "carry": a stemmer targeting the French language.
    -V, --vocab <name>       Path to a CSV file containing allowed vocabulary (or "-" for stdin).
    --vocab-token <col>      Column of vocabulary file containing allowed tokens.
                             [default: token]
    --vocab-token-id <col>   Column of vocabulary file containing a token id to emit in place of the
                             token itself.
    --sep <delim>            If given, the command will output exactly one row per input row,
                             keep the text column and join the tokens using the provided character.
                             We recommend using "§" as a separator.
    --ngrams-sep <delim>     Separator to be use to join ngrams tokens.
                             [default: |]
    --keep-text              Force keeping the text column in output.
    -p, --parallel           Whether to use parallelization to speed up computations.
                             Will automatically select a suitable number of threads to use
                             based on your number of cores. Use -t, --threads if you want to
                             indicate the number of threads yourself.
    -t, --threads <threads>  Parellize computations using this many threads. Use -p, --parallel
                             if you want the number of threads to be automatically chosen instead.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be interpreted
                           as headers.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
