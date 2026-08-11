import Sembla.DSL

sembla_model PartitionBecome (dt := 1.0) where
  partition Band projects population.Person.age where
    low := 0 ..< 1
    high := 1 ..
  box population where
    system Person (rows := 1) where
      age : Int
      status : {present, vacant}
    state Active on Person where
      status := present
    relation bad (band : Band) on Person where
      from Active
      hazard 0.1
      become Band(band)
