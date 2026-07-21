import Std

namespace Sembla.Composition

/-- One expanded primitive occurrence in the frozen V1 source map. -/
structure SourceMapLeafV1 where
  occurrence : String
  definition : String
  instancePath : List String
  displayPath : String
deriving Repr, BEq

/-- Minimal source map emitted by the product linker.  Boundary and hidden
    entries remain empty until PRD 0009 defines their payloads. -/
structure SourceMapV1 where
  schemaVersion : String
  leaves : List SourceMapLeafV1
  boundary : List String
  hidden : List String
deriving Repr, BEq

end Sembla.Composition
