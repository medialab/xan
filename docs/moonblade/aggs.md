# Available aggregation functions

Note that most functions ignore empty values. This said, functions working on
numbers will yield an error when encountering a string that cannot be safely
parsed as a suitable number.

You can always cast values around and force aggregation functions to
consider empty values or make them avoid non-numerical values altogether.

For instance, considering null values when computing a mean is as easy
as `mean(number || 0)`.

Finally, note that expressions returning lists will be understood as multiplexed rows.
This means that computing `cardinality([source, target])`, for instance, will return
the number of nodes in a graph represented by a CSV edge list.

- **all**(*\<expr\>*) -> `bool`: Returns true if all elements returned by given expression are truthy.
- **any**(*\<expr\>*) -> `bool`: Returns true if any of the elements returned by given expression is truthy.
- **approx_cardinality**(*\<expr\>*) -> `int`: Returns the approximate cardinality of the set of values returned by given expression using the HyperLogLog+ algorithm.
- **approx_quantile**(*\<expr\>*, *p*) -> `number`: Returns an approximation of the desired quantile of values returned by given expression using t-digests.
- **argmin**(*\<expr\>*, *\<expr\>?*) -> `any`: Return the index of the row where the first expression is minimized, or the result of the second expression where the first expression is minimized. Ties will be broken by original row index.
- **argmax**(*\<expr\>*, *\<expr\>?*) -> `any`: Return the index of the row where the first expression is maximized, or the result of the second expression where the first expression is maximized. Ties will be broken by original row index.
- **argtop**(*k*, *\<expr\>*, *\<expr\>?*, *separator?*) -> `string`: Find the top k values returned by the first expression and either return the indices of matching rows or the result of the second expression, joined by a pipe character ('|') or by the provided separator. Ties will be broken by original row index.
- **avg**(*\<expr\>*) -> `number`: Average of numerical values. Same as `mean`.
- **cardinality**(*\<expr\>*) -> `number`: Number of distinct values returned by given expression.
- **correlation**(*\<expr\>*, *\<expr\>*) -> `number`: Return the correlation (covariance divided by the product of standard deviations) of series represented by the two given expressions.
- **count**(*\<expr\>?*) -> `number`: Count the number of truthy values returned by given expression. Expression can also be omitted to count all rows.
- **count_seconds**(*\<expr\>*) -> `number`: Count the number of seconds between earliest and latest datetime returned by given expression.
- **count_hours**(*\<expr\>*) -> `number`: Count the number of hours between earliest and latest datetime returned by given expression.
- **count_days**(*\<expr\>*) -> `number`: Count the number of days between earliest and latest datetime returned by given expression.
- **count_years**(*\<expr\>*) -> `number`: Count the number of years between earliest and latest datetime returned by given expression.
- **covariance**(*\<expr\>*, *\<expr\>*) -> `number`: Return the population covariance of series represented by the two given expressions. Same as `covariance_pop`.
- **covariance_pop**(*\<expr\>*, *\<expr\>*) -> `number`: Return the population covariance of series represented by the two given expressions. Same as `covariance`.
- **covariance_sample**(*\<expr\>*, *\<expr\>*) -> `number`: Return the sample covariance of series represented by the two given expressions.
- **distinct_values**(*\<expr\>*, *separator?*) -> `string`: List of sorted distinct values joined by a pipe character ('|') by default or by the provided separator.
- **earliest**(*\<expr\>*) -> `datetime`: Earliest datetime returned by given expression.
- **first**(*\<expr\>*) -> `string`: Return first seen non empty element of the values returned by the given expression.
- **latest**(*\<expr\>*) -> `datetime`: Latest datetime returned by given expression.
- **last**(*\<expr\>*) -> `string`: Return last seen non empty element of the values returned by the given expression.
- **lex_first**(*\<expr\>*) -> `string`: Return first string in lexicographical order.
- **lex_last**(*\<expr\>*) -> `string`: Return last string in lexicographical order.
- **min**(*\<expr\>*) -> `number`: Minimum numerical value.
- **max**(*\<expr\>*) -> `number`: Maximum numerical value.
- **mean**(*\<expr\>*) -> `number`: Mean of numerical values. Same as `avg`.
- **median**(*\<expr\>*) -> `number`: Median of numerical values, interpolating on even counts.
- **median_high**(*\<expr\>*) -> `number`: Median of numerical values, returning higher value on even counts.
- **median_low**(*\<expr\>*) -> `number`: Median of numerical values, returning lower value on even counts.
- **mode**(*\<expr\>*) -> `string`: Value appearing the most, breaking ties arbitrarily in favor of the first value in lexicographical order.
- **most_common**(*k*, *\<expr\>*, *separator?*) -> `string`: List of top k most common values returned by expression joined by a pipe character ('|') or by the provided separator. Ties will be broken by lexicographical order.
- **most_common_counts**(*k*, *\<expr\>*, *separator?*) -> `string`: List of top k most common counts returned by expression joined by a pipe character ('|') or by the provided separator. Ties will be broken by lexicographical order.
- **percentage**(*\<expr\>*) -> `string`: Return the percentage of truthy values returned by expression.
- **quantile**(*\<expr\>*, *q*) -> `number`: Return the desired quantile of numerical values.
- **q1**(*\<expr\>*) -> `number`: Return the first quartile of numerical values.
- **q2**(*\<expr\>*) -> `number`: Return the second quartile of numerical values.
- **q3**(*\<expr\>*) -> `number`: Return the third quartile of numerical values.
- **ratio**(*\<expr\>*) -> `number`: Return the ratio of truthy values returned by expression.
- **rms**(*\<expr\>*) -> `number`: Return the Root Mean Square of numerical values.
- **stddev**(*\<expr\>*) -> `number`: Population standard deviation. Same as `stddev_pop`.
- **stddev_pop**(*\<expr\>*) -> `number`: Population standard deviation. Same as `stddev`.
- **stddev_sample**(*\<expr\>*) -> `number`: Sample standard deviation (i.e. using Bessel's correction).
- **sum**(*\<expr\>*) -> `number`: Sum of numerical values. Will return nothing if the sum overflows. Uses the Kahan-Babuska routine for precise float summation.
- **top**(*k*, *\<expr\>*, *separator?*) -> `any`: Find the top k values returned by the expression and join them by a pipe character ('|') or by the provided separator. Ties will be broken by original row index.
- **type**(*\<expr\>*) -> `string`: Best type description for seen values.
- **types**(*\<expr\>*) -> `string`: Sorted list, pipe-separated, of all the types seen in the values.
- **values**(*\<expr\>*, *separator?*) -> `string`: List of values joined by a pipe character ('|') by default or by the provided separator.
- **var**(*\<expr\>*) -> `number`: Population variance. Same as `var_pop`.
- **var_pop**(*\<expr\>*) -> `number`: Population variance. Same as `var`.
- **var_sample**(*\<expr\>*) -> `number`: Sample variance (i.e. using Bessel's correction).
