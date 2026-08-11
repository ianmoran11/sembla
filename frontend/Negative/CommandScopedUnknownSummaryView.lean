import Sembla.DSL
open Sembla.DSL
sembla_model ScopedUnknownSummaryView (dt := 1.0) where
  box b where
    system A (rows := 1)
    views A where
      v := count
  summaries b where
    s := max missing
