import Sembla.Json

namespace Sembla.ScientificTests
open Sembla.IR

private def probe : Model :=
  Model.mk "numeric_probe" 1.2345678901234567
    [ParamDecl.mk "x" .real (.real (-9.876543210987654))
      (some (Prior.mk .uniform [0.000000123456789, 1e300]))]
    [] []

-- These values are unrelated to either checked-in fixture.  Building this
-- module proves that serialization retains all represented decimal digits and
-- handles both small and very large finite scientific values generally.
#guard toJson probe ==
  "{\"name\":\"numeric_probe\",\"dt\":12345678901234567e-16,\"params\":[{\"name\":\"x\",\"ty\":\"real\",\"default\":{\"kind\":\"real\",\"value\":-9876543210987654e-15},\"prior\":{\"family\":\"uniform\",\"args\":[123456789e-15,1e300]}}],\"boxes\":[],\"wires\":[]}\n"

end Sembla.ScientificTests
