import Std

namespace Sembla.Composition

/-- One expanded primitive occurrence in the frozen V1 source map. -/
structure SourceMapLeafV1 where
  occurrence : String
  definition : String
  instancePath : List String
  displayPath : String
deriving Repr, BEq

/-- One root boundary alias resolved transitively to a primitive leaf. -/
structure SourceMapBoundaryV1 where
  outer : String
  leaf : String
  port : String
  path : List String
deriving Repr, BEq

/-- One root-owned hidden direct-child boundary port. -/
structure SourceMapHiddenV1 where
  instance_ : String
  port : String
deriving Repr, BEq

structure SourceMapV1 where
  schemaVersion : String
  leaves : List SourceMapLeafV1
  boundary : List SourceMapBoundaryV1
  hidden : List SourceMapHiddenV1
deriving Repr, BEq

end Sembla.Composition
