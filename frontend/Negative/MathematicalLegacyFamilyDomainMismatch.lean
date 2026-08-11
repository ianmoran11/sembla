import Sembla.DSL

sembla_model LegacyFamilyDomainMismatch (dt := 1.0) where
  index area := {a, b}
  domain other := {a, b}
  param rate[area] : ℝ where
    [a] := 0.1
    [b] := 0.2
  box population where
    system Person (rows := 1) where
      status : {present, vacant}
    transition bad[area : other] on Person where
      guard status = present
      hazard rate[area]
      set status := vacant
