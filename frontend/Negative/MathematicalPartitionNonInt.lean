import Sembla.DSL

sembla_model PartitionNonInt (dt := 1.0) where
  partition Band projects population.Person.score where
    low := 0 ..< 1
    high := 1 ..
  box population where
    system Person (rows := 1) where
      score : ℝ
