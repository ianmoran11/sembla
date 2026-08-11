import Sembla.DSL

sembla_model AliasPredicateBecome (dt := 1.0) where
  domain UnitDomain := {only}
  box population where
    system Person (rows := 1) where
      status : {present, vacant}
    state Matching on Person where
      match status = present
    relation bad (unit : UnitDomain) on Person where
      from Matching
      hazard 0.1
      become Matching
