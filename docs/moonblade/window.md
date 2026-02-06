# Available window aggregation functions

- **cume_dist**(*\<expr\>*) -> `number`: Returns the cumulative distribution of numbers yielded by given expression. Beware, as this requires buffering whole file or group.
- **cummax**(*\<expr\>*) -> `number`: Returns the cumulative maximum of the numbers yielded by given expression.
- **cummin**(*\<expr\>*) -> `number`: Returns the cumulative minimum of the numbers yielded by given expression.
- **cumsum**(*\<expr\>*) -> `number`: Returns the cumulative sum of the numbers yielded by given expression.
- **dense_rank**(*\<expr\>*) -> `number`: Returns the dense rank (there will be no gaps, but ties remain possible for a same rank) of numbers yielded by given expression. Beware, as this requires buffering whole file or group.
- **frac**(*\<expr\>*, *decimals?*) -> `number`: Returns the fraction represented by numbers yielded by given expression over the total sum of them. Beware, as this requires buffering whole file or group.
- **lag**(*\<expr\>*, *steps?*, *\<expr\>?*) -> `any`: Returns a value yielded by given expression, lagged by n steps or 1 step by default. Can take a second expression after the number of steps to return a default value for rows that come before first lagged value.
- **lead**(*\<expr\>*, *steps?*, *\<expr\>?*) -> `any`: Returns a value yielded by given expression, leading by n steps or 1 step by default. Can take a second expression after the number of steps to return a default value for rows that come after last lead value.
- **rank**(*\<expr\>*) -> `number`: Returns the arbitrary rank (ties will will be broken in input order) of numbers yielded by given expression. Beware, as this requires buffering whole file or group.
- **rolling_avg**(*window_size*, *\<expr\>*) -> `number`: Returns the rolling average in given window size of numbers yielded by given expression. Same as `rolling_mean`.
- **rolling_mean**(*window_size*, *\<expr\>*) -> `number`: Returns the rolling mean in given window size of numbers yielded by given expression. Same as `rolling_avg`.
- **rolling_stddev**(*window_size*, *\<expr\>*) -> `number`: Returns the rolling population standard deviation in given window size of numbers yielded by given expression.
- **rolling_sum**(*window_size*, *\<expr\>*) -> `number`: Returns the rolling sum in given window size of numbers yielded by given expression.
- **rolling_var**(*window_size*, *\<expr\>*) -> `number`: Returns the rolling population variance in given window size of numbers yielded by given expression.
- **row_index**() -> `number`: Returns the 0-based row index.
- **row_number**() -> `number`: Returns the 1-based row number.
