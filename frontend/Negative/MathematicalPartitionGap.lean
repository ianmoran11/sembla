import Sembla.DSL

sembla_model PartitionGap (dt := 1.0) where
  partition Band projects population.Person.age where
    low := 0 ..< 1
    high := 2 ..
  box population where
    system Person (rows := 1) where
      age : Int
