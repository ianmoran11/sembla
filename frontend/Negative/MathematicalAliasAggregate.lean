import Sembla.DSL

sembla_model AliasAggregate (dt := 1.0) where
  box population where
    system Resource (rows := 1)
    system Person (rows := 1) where
      resource : Resource
      score : Int
    state Active on Person where
      score := sizeBy resource
