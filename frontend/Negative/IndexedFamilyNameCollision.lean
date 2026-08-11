import Sembla.DSL

sembla_model FamilyNameCollision (dt := 1.0) where
  index label := {fooBar, foo_bar}
  param beta[label] : ℝ where
    [fooBar] := 0.1
    [foo_bar] := 0.2
