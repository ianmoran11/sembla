import Sembla.Composition.SurfaceModels
open Sembla.Composition.SurfaceModels

sembla_composition invalidSummaryPath
    (name := "invalid_summary_path") (dt := 0.25) where
  root TwoRegions
  summary bad := max north.1.I
