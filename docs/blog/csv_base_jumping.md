# Cursed engineering: jumping randomly through CSV files without hurting yourself

<p align="center">
  <img src="./img/csv-base-jumping.png" alt="csv-base-jumping">
</p>

TODO: tl;dr, xan link

## Summary

TODO...

## Is that even dangerous?

Let's say you have a big CSV file and you jump to a random byte within it, would you be able to find where next row will start?

At first glance you might think this is an easy problem, just read bytes until you find a line break and you are done, right?

Before we go further, let's consider this short excerpt from Hamlet:

```txt
If thou art privy to thy country's fate,
Which, happily, foreknowing may avoid, O, speak!
Or if thou hast uphoarded in thy life
Extorted treasure in the womb of earth,
For which, they say, you spirits oft walk in death,
Speak of it: stay, and speak! Stop it, Marcellus.
```

I am unfortunately happy to report that those verses are perfectly valid CSV data in themselves:

| 0                                       | 1                  | 2                             | 3   | 4      |
| --------------------------------------- | ------------------ | ----------------------------- | --- | ------ |
| If thou art privy to thy country's fate |                    |                               |     |        |
| Which                                   | happily            | foreknowing may avoid         | O   | speak! |
| Or if thou hast uphoarded in thy life   |                    |                               |     |        |
| Extorted treasure in the womb of earth  |                    |                               |     |        |
| For which                               | they say           | you spirits oft walk in death |     |        |
| Speak of it: stay                       | and speak! Stop it | Marcellus.                    |     |        |

Now let's consider a more realistic scenario where we have a CSV file with a column containing raw text. The CSV format obviously knows how to accomodate this without becoming structurally unsound. This is done through "quoting": any cell containing either commas, double quotes or newline characters will be quoted using double quotes, and any double quote within will be doubled (`"` would become `""`). For instance, the following CSV data:

```txt
verse_group,text,quality
1,"If thou art privy to thy country's fate,
Which, happily, foreknowing may avoid, O, speak!",10
2,"Or if thou hast uphoarded in thy life
Extorted treasure in the womb of earth,",5
3,"For which, they say, you spirits oft walk in death,
Speak of it: stay, and speak! Stop it, Marcellus.",50
```

Would translate to the following table:

| verse_group | text                                                                                                   | quality |
| ----------- | ------------------------------------------------------------------------------------------------------ | ------- |
| 1           | If thou art privy to thy country's fate,\nWhich, happily, foreknowing may avoid, O, speak!             | 10      |
| 2           | Or if thou hast uphoarded in thy life\nExtorted treasure in the womb of earth,                         | 5       |
| 3           | For which, they say, you spirits oft walk in death,\nSpeak of it: stay, and speak! Stop it, Marcellus. | 50      |

Finally, to add insult to injury, notice how a CSV cell is perfectly able to encase perfectly valid CSV data through quoting. As an example, behold this shameful recursive beast of a CSV file:

```txt
table,data
1,"name,surname
john,landis
lucy,gregor"
2,"id,color,score
1,blue,56
2,red,67
3,yellow,6"
```

| table | data                                            |
| ----- | ----------------------------------------------- |
| 1     | name,surname\njohn,landis\nlucy,gregor          |
| 2     | id,color,score\n1,blue,56\n2,red,67\n3,yellow,6 |

Now let's come back to our jumping thought experiment: the issue here is that, if you jump to a random byte of a CSV file, you cannot know whether you landed in a quoted cell or not. So, if you read ahead and find a line break, is it delineating a CSV row, or is just allowed here because we stand in a quoted cell? And if you find a double quote? Are you opening a quoted cell or are you closing one?

For instance, here we jumped into a quoted section:

```html
table,data
1,"name,surname
john,<there>landis
lucy,gregor"
2,"id,color,score
1,blue,56
2,red,67
3,yellow,6"
```

But here we jumped into an unquoted section:

```html
table,data
1,"name,surname
john,landis
lucy,gregor"
2,"id,color,score
1,blue,5<there>6
2,red,67
3,yellow,6"
```

This seems helpless. But I would not be writing about this issue if I had no solution to offer, albeit a slightly unhinged one.

## Our lord and savior: statistics

Real-life CSV data is *usually* consistent. What I mean by that is that tabular data often has a fixed number of columns. Indeed, rows suddenly demonstrating an inconsistent number of columns are typically frowned upon. What's more, columns often hold homogeneous data types: integers, floating point numbers, raw text, dates etc. We can of course leverage this consistency.

So now, before doing any reckless jumping, let's start by analyzing the beginning of our CSV file to record some statistics that will be useful down the line.

We need to sample a fixed but sufficient number of rows (`128` is a good place to start, to adhere to computer science's justified fetichism regarding base 2), in order to record the following information:

* the number of columns of the file
* the maximum size, in bytes, of all sampled rows
* a profile of the columns, that is to say a vector of the average size in bytes of all sampled cells from each column

Here is an example of what you might get:

```txt
Sample {
    columns: 17,
    max_record_size: 19131,
    fields_mean_sizes: [
        150.265625,
        0.28125,
        87.171875,
        208.1640625,
        22.515625,
        53.3203125,
        24.21875,
        24.21875,
        17.7734375,
        4.84375,
        15.5625,
        6.875,
        103.7265625,
        4.03125,
        4908.6875,
        342.390625,
        246.8046875,
    ]
}
```

Notice how some columns seem typically larger than others?

Anyway, we now have what we need to be able to jump safely.

## Donning the wingsuit

jumping, reading normally up to n times, comparing columns, tie breaker cosine

## Why though?

TODO: subsections for toc

### Single-pass map-reduce parallelization over CSV files

indexing, gzi nod

### Cursed sampling

### Binary search

## Caveat emptor

<!-- ---

refactor: les rows ont une taille homogene aussi

complexity analysis, at least constant bound + seek-bound + constant memory
only work if you can seek of course

caveat emptor
caveats: does not work properly at the end of the file nor at the beginning (we have the sample anyway), does not work on small files, but eh..., there are silly cases where this method does not work, they are not useful for our purpose. probabilistic method, explain about the pyramid or holey file

links to seeker

csv data is usually consistent, same number of columns, homegeneous data types

*Notes*

- grep can emulate this also, this is easy
- which `xan` commands benefited from this
- link to the Seeker in simd-csv -->