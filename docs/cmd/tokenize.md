<!-- Generated -->
# xan tokenize

```txt
Tokenize the given text column by splitting it either into words, sentences
or paragraphs.

# tokenize words

Tokenize the given text column by splitting it into word pieces (think
words, numbers, hashtags etc.).

This tokenizer is able to distinguish between the following types of
tokens (that you can filter using --keep and --drop):
    "word", "number", "hashtag", "mention", "emoji",
    "punct", "url" and "email"

The command will by default return one row per row in the input file, with
the tokens added in a new "tokens" column containing the processed and filtered
tokens joined by a space (or any character given to --sep).

However, when giving a column name to -T, --token-type, the command will
instead emit one row per token with the token in a new "token" column, along
with a new column containing the token's type.

This subcommand also exposes many ways to filter and process the resulting
tokens as well as ways to refine a vocabulary iteratively in tandem with
the "xan vocab" command.

# tokenize sentences

Tokenize the given text by splitting it into sentences, emitting one row per
sentence with a new "sentence" column at the end.

# tokenize paragraphs

Tokenize the given text by splitting it into paragraphs, emitting one row per
paragraph, with a new "paragraph" column at the end.

---

Note that the command will always drop the text column from the
output unless you pass --keep-text to the command.

Tips:

You can easily pipe the command into "xan vocab" to create a vocabulary:
    $ xan tokenize words text file.csv | xan vocab token token > vocab.csv

You can easily keep the tokens in a separate file using the "tee" command:
    $ xan tokenize words text file.csv | tee tokens.csv | xan vocab token token > vocab.csv

Usage:
    xan tokenize words [options] <column> [<input>]
    xan tokenize sentences [options] <column> [<input>]
    xan tokenize paragraphs [options] <column> [<input>]
    xan tokenize --help

tokenize options:
    -c, --column <name>      Name for the token column. Will default to "tokens", "token"
                             when -T/--token-type is provided, "paragraphs" or "sentences".
    -p, --parallel           Whether to use parallelization to speed up computations.
                             Will automatically select a suitable number of threads to use
                             based on your number of cores. Use -t, --threads if you want to
                             indicate the number of threads yourself.
    -t, --threads <threads>  Parellize computations using this many threads. Use -p, --parallel
                             if you want the number of threads to be automatically chosen instead.
    --keep-text              Force keeping the text column in the output.

tokenize words options:
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
    --sep <delim>            Character used to join tokens in the output cells. Will default
                             to a space.
    --ngrams-sep <delim>     Separator to be use to join ngrams tokens.
                             [default: §]

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be interpreted
                           as headers.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
