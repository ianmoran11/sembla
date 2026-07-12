import Sembla.DSL
open Sembla.IR Sembla.DSL

def reorderedOutput : Model := model% "order" step(1.0) where
  params []
  boxes [box demo where
    systems [system Person as "person" rows(1) where [attr x : Int]]
    inputs []
    transitions []
    outputs [output stats {a : Int, b : Int} from Person fields [
      field b := count where x = x,
      field a := count where x = x]]]
  wires []

private def firstOutputNames (model : Model) : List String :=
  match model.boxes.head? with
  | none => []
  | some modelBox =>
      match modelBox.outputs.head? with
      | none => []
      | some outputDecl =>
          match outputDecl.builder with
          | .perTable _ builderFields => builderFields.map (·.name)

#guard firstOutputNames reorderedOutput == ["a", "b"]
