import Sembla.DSL

sembla_model DuplicateAlias (dt := 1.0) where
  box population where
    system Person (rows := 1) where
      status : {present, vacant}
    state Active on Person where
      status := present
    state Active on Person where
      status := vacant
