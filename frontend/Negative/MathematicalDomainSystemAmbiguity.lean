import Sembla.DSL

sembla_model DomainSystemAmbiguity (dt := 1.0) where
  domain Person := {only}
  box population where
    system Person (rows := 1)
