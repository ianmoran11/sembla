import Sembla.DSL

sembla_model TransitionNameCollision (dt := 1.0) where
  index label := {fooBar, foo_bar}
  box population where
    system Person (rows := 1) where
      health : {S, I}
      label : {fooBar, foo_bar}
    infect[label] on Person : health: S →[0.1] I
