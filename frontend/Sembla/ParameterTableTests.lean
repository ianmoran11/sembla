import Sembla.ParameterTable

namespace Sembla.ParameterTableTests
open Sembla ParameterTable

private def csvText : String := include_str "TestData/IndexedFamilies/beta.csv"
private def jsonText : String := include_str "TestData/IndexedFamilies/beta.json"
private def dimensions := ["age", "sex"]
private def keys : List (List IndexMember) :=
  [ [.int 0, .enum "male"]
  , [.int 0, .enum "female"]
  , [.int 1, .enum "male"]
  , [.int 1, .enum "female"]
  ]

private def defaultsAt (table : RawTable) : List (Option String) :=
  keys.map fun key => (table.cells.find? (·.key == key)).map (·.defaultText)

private def priorsAt (table : RawTable) : List (Option (Option RawPrior)) :=
  keys.map fun key => (table.cells.find? (·.key == key)).map (·.prior)

private def parsedCsv : Option RawTable := (parseCsv dimensions csvText).toOption
private def parsedJson : Option RawTable := (parseJson dimensions jsonText).toOption

#guard parsedCsv.map defaultsAt == some
  [some "0.10", some "0.123456789012345678901234567890", some "6.25e-2", some "0.4"]
#guard parsedCsv.map priorsAt == some
  [some (some { family := .logNormal, args := ("-2.302585092994046", "2.5e-1") }),
    some none, some none, some none]
#guard parsedJson.map (fun table => (defaultsAt table, priorsAt table)) ==
  parsedCsv.map (fun table => (defaultsAt table, priorsAt table))
#guard Sembla.Hash.sha256HexOfString csvText ==
  "613195059792a4054b01c98e083d375865c7cfaf14515285cdc90b6cf04ce3fc"
#guard Sembla.Hash.sha256HexOfString jsonText ==
  "8236ce78ea61f379f10919456a06b231dac5d22daadffc74ea12b6e0b30a30b7"

private def mixedCsvText : String :=
  include_str "TestData/IndexedFamilies/mixed-priors-v2.csv"
private def mixedJsonText : String :=
  include_str "TestData/IndexedFamilies/mixed-priors-v2.json"
private def mixedPriors (table : RawTable) : List (Option RawPrior) :=
  table.cells.map (·.prior)

#guard (parseCsvV2 ["group"] mixedCsvText).toOption.map mixedPriors == some
  [ some { family := .normal, args := ("1.0", "0.25") }
  , some { family := .logNormal, args := ("0.6931471805599453", "0.5") }
  ]
#guard (parseJson ["group"] mixedJsonText).toOption.map mixedPriors ==
  (parseCsvV2 ["group"] mixedCsvText).toOption.map mixedPriors

private def v1CsvNormalRejected := parseCsv ["group"] mixedCsvText
private def v1NormalRejected := parseJson ["group"]
  "{\"schema_version\":\"sembla.parameter-family/v1\",\"dimensions\":[\"group\"],\"cells\":[{\"key\":{\"group\":\"a\"},\"default\":\"1.0\",\"prior\":{\"family\":\"normal\",\"args\":[\"1\",\"0.1\"]}}]}"
private def csvQuotedRecords : Option (List (List String)) :=
  (parseCsvRecords "a,b\r\n\"x,y\",\"z\"\"q\"\r\n").toOption
private def failed : Except String α → Bool
  | .error _ => true
  | .ok _ => false

#guard failed v1CsvNormalRejected
#guard failed v1NormalRejected
#guard csvQuotedRecords == some [["a", "b"], ["x,y", "z\"q"]]

private def rejectedCsvTables : List String :=
  [ "sex,age,default,prior_family,prior_arg_1,prior_arg_2\n"
  , "age,sex,extra,default,prior_family,prior_arg_1,prior_arg_2\n"
  ]

#guard rejectedCsvTables.all fun input => failed (parseCsv dimensions input)

private def rejectedJsonTables : List String :=
  [ -- Unknown top-level field.
    "{\"schema_version\":\"sembla.parameter-family/v1\",\"dimensions\":[\"age\",\"sex\"],\"cells\":[],\"extra\":null}"
    -- Unknown cell field.
  , "{\"schema_version\":\"sembla.parameter-family/v1\",\"dimensions\":[\"age\",\"sex\"],\"cells\":[{\"key\":{\"age\":0,\"sex\":\"male\"},\"default\":\"0.1\",\"prior\":null,\"extra\":null}]}"
    -- Unknown key field.
  , "{\"schema_version\":\"sembla.parameter-family/v1\",\"dimensions\":[\"age\",\"sex\"],\"cells\":[{\"key\":{\"age\":0,\"sex\":\"male\",\"extra\":0},\"default\":\"0.1\",\"prior\":null}]}"
    -- Unknown prior field.
  , "{\"schema_version\":\"sembla.parameter-family/v1\",\"dimensions\":[\"age\",\"sex\"],\"cells\":[{\"key\":{\"age\":0,\"sex\":\"male\"},\"default\":\"0.1\",\"prior\":{\"family\":\"log_normal\",\"args\":[\"0\",\"1\"],\"extra\":null}}]}"
    -- Numeric default rather than an exact string.
  , "{\"schema_version\":\"sembla.parameter-family/v1\",\"dimensions\":[\"age\",\"sex\"],\"cells\":[{\"key\":{\"age\":0,\"sex\":\"male\"},\"default\":0.1,\"prior\":null}]}"
    -- Numeric prior argument rather than an exact string.
  , "{\"schema_version\":\"sembla.parameter-family/v1\",\"dimensions\":[\"age\",\"sex\"],\"cells\":[{\"key\":{\"age\":0,\"sex\":\"male\"},\"default\":\"0.1\",\"prior\":{\"family\":\"log_normal\",\"args\":[0,\"1\"]}}]}"
    -- Dimensions do not match the declaration.
  , "{\"schema_version\":\"sembla.parameter-family/v1\",\"dimensions\":[\"sex\",\"age\"],\"cells\":[]}"
  ]

#guard rejectedJsonTables.all fun input => failed (parseJson dimensions input)
#guard failed (validateSha256 "ABC")
#guard failed (validateSha256
  "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA")

end Sembla.ParameterTableTests
