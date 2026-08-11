import Sembla.DSL
open Sembla.DSL

sembla_model Bad (dt := 1.0) where
  partition Active projects population.Person.age where
    young := 0 ..
  box population where
    system Person (rows := 1) where
      age : Int
      status : {active, inactive}
    state Active on Person where
      status := active
