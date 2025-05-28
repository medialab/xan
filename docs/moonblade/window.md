# Available window aggregation functions

- **cummax**(*\<expr\>*) -> `number`: Returns the cumulative maximum of the numbers yielded by given expression.
- **cummin**(*\<expr\>*) -> `number`: Returns the cumulative minimum of the numbers yielded by given expression.
- **cumsum**(*\<expr\>*) -> `number`: Returns the cumulative sum of the numbers yielded by given expression.
- **lag**(*\<expr\>*, *steps?*) -> `any`: Returns a value yielded by given expression, lagged by n steps or 1 step by default.
- **lead**(*\<expr\>*, *steps?*) -> `any`: Returns a value yielded by given expression, leading by n steps or 1 step by default.
- **rolling_avg**(*window_size*, *\<expr\>*) -> `number`: Returns the rolling average in given window size of numbers yielded by given expression. Same as `rolling_mean`.
- **rolling_mean**(*window_size*, *\<expr\>*) -> `number`: Returns the rolling mean in given window size of numbers yielded by given expression. Same as `rolling_avg`.
- **rolling_stddev**(*window_size*, *\<expr\>*) -> `number`: Returns the rolling population standard deviation in given window size of numbers yielded by given expression.
- **rolling_sum**(*window_size*, *\<expr\>*) -> `number`: Returns the rolling sum in given window size of numbers yielded by given expression.
- **rolling_var**(*window_size*, *\<expr\>*) -> `number`: Returns the rolling population variance in given window size of numbers yielded by given expression.
- **row_index**() -> `number`: Returns the 0-based row index.
- **row_number**() -> `number`: Returns the 1-based row number.
