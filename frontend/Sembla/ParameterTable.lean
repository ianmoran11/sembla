import Lean.Data.Json
import Sembla.Hash

/-!
Strict, frontend-only support for indexed parameter tables.

The public DSL elaborator owns domain/type checking and IR lowering. This module
only decodes exact external table bytes into an order-independent raw cell list.
-/
namespace Sembla.ParameterTable

inductive IndexMember where
  | int (value : Nat)
  | enum (value : String)
deriving Repr, BEq

inductive IndexDomain where
  | range (lower upper : Nat)
  | enumeration (members : List String)
deriving Repr, BEq

def IndexMember.component : IndexMember → String
  | .int value => toString value
  | .enum value => value

def IndexDomain.members : IndexDomain → List IndexMember
  | .range lower upper => (List.range (upper - lower + 1)).map fun offset => .int (lower + offset)
  | .enumeration values => values.map .enum

inductive RawPriorFamily where
  | normal
  | logNormal
deriving Repr, BEq

structure RawPrior where
  family : RawPriorFamily
  args : String × String
deriving Repr, BEq

structure RawCell where
  key : List IndexMember
  defaultText : String
  prior : Option RawPrior
  location : String
deriving Repr, BEq

structure RawTable where
  dimensions : List String
  cells : List RawCell
deriving Repr, BEq

inductive Format where
  | csv
  | csvV2
  | json
deriving Repr, BEq

private def validHexChar (c : Char) : Bool :=
  ('0' ≤ c && c ≤ '9') || ('a' ≤ c && c ≤ 'f')

def validateSha256 (digest : String) : Except String Unit := do
  unless digest.length == 64 && digest.toList.all validHexChar do
    throw "SHA-256 must contain exactly 64 lowercase hexadecimal characters"

private def finishCsvField (field : List Char) : String := String.mk field.reverse

/-- RFC-4180-style records with quoted fields, doubled quotes, LF, and CRLF. -/
def parseCsvRecords (input : String) : Except String (List (List String)) := do
  let chars := input.toList.toArray
  let mut index := 0
  let mut quoted := false
  let mut justClosed := false
  let mut field : List Char := []
  let mut row : List String := []
  let mut rows : List (List String) := []
  while index < chars.size do
    let c := chars[index]!
    if quoted then
      if c == '"' then
        if index + 1 < chars.size && chars[index + 1]! == '"' then
          field := '"' :: field
          index := index + 2
        else
          quoted := false
          justClosed := true
          index := index + 1
      else
        field := c :: field
        index := index + 1
    else if justClosed then
      if c == ',' then
        row := row ++ [finishCsvField field]
        field := []
        justClosed := false
        index := index + 1
      else if c == '\n' then
        row := row ++ [finishCsvField field]
        field := []
        rows := rows ++ [row]
        row := []
        justClosed := false
        index := index + 1
      else if c == '\r' then
        if index + 1 < chars.size && chars[index + 1]! == '\n' then
          row := row ++ [finishCsvField field]
          field := []
          rows := rows ++ [row]
          row := []
          justClosed := false
          index := index + 2
        else throw "CSV: carriage return must be followed by newline"
      else throw "CSV: unexpected character after closing quote"
    else if c == '"' then
      if field.isEmpty then
        quoted := true
        index := index + 1
      else throw "CSV: quote must begin a field"
    else if c == ',' then
      row := row ++ [finishCsvField field]
      field := []
      index := index + 1
    else if c == '\n' then
      row := row ++ [finishCsvField field]
      field := []
      rows := rows ++ [row]
      row := []
      index := index + 1
    else if c == '\r' then
      if index + 1 < chars.size && chars[index + 1]! == '\n' then
        row := row ++ [finishCsvField field]
        field := []
        rows := rows ++ [row]
        row := []
        index := index + 2
      else throw "CSV: carriage return must be followed by newline"
    else
      field := c :: field
      index := index + 1
  if quoted then throw "CSV: unterminated quoted field"
  if !field.isEmpty || !row.isEmpty || justClosed then
    row := row ++ [finishCsvField field]
    rows := rows ++ [row]
  pure rows

private def enumerate (values : List α) : List (α × Nat) :=
  let rec loop (index : Nat) : List α → List (α × Nat)
    | [] => []
    | value :: rest => (value, index) :: loop (index + 1) rest
  loop 0 values

private def parseNatMember (where_ value : String) : Except String IndexMember :=
  match value.toNat? with
  | some number => pure (.int number)
  | none => throw s!"{where_}: expected a natural-number index member"

private def csvMember (where_ : String) (value : String) : Except String IndexMember :=
  match value.toNat? with
  | some number => pure (.int number)
  | none => if value.isEmpty then throw s!"{where_}: index member must not be empty" else pure (.enum value)

private def parseCsvWithVersion (allowNormal : Bool) (expectedDimensions : List String)
    (input : String) : Except String RawTable := do
  let rows ← parseCsvRecords input
  let header ← match rows with
    | [] => throw "CSV: missing header"
    | header :: _ => pure header
  let expectedHeader := expectedDimensions ++ ["default", "prior_family", "prior_arg_1", "prior_arg_2"]
  unless header == expectedHeader do
    throw s!"CSV: expected header '{String.intercalate "," expectedHeader}'"
  let mut cells : List RawCell := []
  for (row, offset) in enumerate (rows.drop 1) do
    let line := offset + 2
    unless row.length == expectedHeader.length do
      throw s!"CSV row {line}: expected {expectedHeader.length} columns, found {row.length}"
    let keyTexts := row.take expectedDimensions.length
    let key ← enumerate keyTexts |>.mapM fun (value, column) => csvMember s!"CSV row {line}, column {column + 1}" value
    let defaultText := row.getD expectedDimensions.length ""
    if defaultText.isEmpty then throw s!"CSV row {line}: default must not be empty"
    let family := row.getD (expectedDimensions.length + 1) ""
    let arg1 := row.getD (expectedDimensions.length + 2) ""
    let arg2 := row.getD (expectedDimensions.length + 3) ""
    let prior ← if family.isEmpty && arg1.isEmpty && arg2.isEmpty then pure none
      else if family == "log_normal" && !arg1.isEmpty && !arg2.isEmpty then
        pure (some { family := .logNormal, args := (arg1, arg2) })
      else if allowNormal && family == "normal" && !arg1.isEmpty && !arg2.isEmpty then
        pure (some { family := .normal, args := (arg1, arg2) })
      else if allowNormal then
        throw s!"CSV row {line}: prior columns must be empty or use 'normal'/'log_normal' with exactly two arguments"
      else
        throw s!"CSV row {line}: prior columns must be empty or use 'log_normal' with exactly two arguments"
    cells := cells ++ [{ key, defaultText, prior, location := s!"CSV row {line}" }]
  pure { dimensions := expectedDimensions, cells }

/-- The original unversioned CSV contract: LogNormal is the only prior family. -/
def parseCsv (expectedDimensions : List String) (input : String) : Except String RawTable :=
  parseCsvWithVersion false expectedDimensions input

/-- Explicit `sembla.parameter-family/v2` CSV mode: exactly Normal and LogNormal. -/
def parseCsvV2 (expectedDimensions : List String) (input : String) : Except String RawTable :=
  parseCsvWithVersion true expectedDimensions input

private abbrev JsonObject := Lean.RBNode String (fun _ => Lean.Json)
private def expectObject (path : String) : Lean.Json → Except String JsonObject
  | .obj fields => pure fields
  | _ => throw s!"{path}: expected object"
private def checkFields (path : String) (fields : JsonObject) (allowed : List String) : Except String Unit :=
  fields.foldM (fun _ key _ => if allowed.contains key then pure () else throw s!"{path}: unknown field '{key}'") ()
private def required (path name : String) (fields : JsonObject) : Except String Lean.Json :=
  match fields.find compare name with | some value => pure value | none => throw s!"{path}.{name}: missing required field"
private def expectString (path : String) : Lean.Json → Except String String
  | .str value => pure value
  | _ => throw s!"{path}: expected string"
private def expectArray (path : String) : Lean.Json → Except String (List Lean.Json)
  | .arr values => pure values.toList
  | _ => throw s!"{path}: expected array"
private def jsonMember (path : String) : Lean.Json → Except String IndexMember
  | .str value => if value.isEmpty then throw s!"{path}: index member must not be empty" else pure (.enum value)
  | value => return .int (← value.getNat?.mapError fun _ => s!"{path}: expected string or natural number")

private def decodePrior (version path : String) : Lean.Json → Except String (Option RawPrior)
  | .null => pure none
  | value => do
      let fields ← expectObject path value
      checkFields path fields ["family", "args"]
      let familyText ← expectString s!"{path}.family" (← required path "family" fields)
      let family ← match version, familyText with
        | "sembla.parameter-family/v1", "log_normal"
        | "sembla.parameter-family/v2", "log_normal" => pure RawPriorFamily.logNormal
        | "sembla.parameter-family/v2", "normal" => pure RawPriorFamily.normal
        | "sembla.parameter-family/v1", _ => throw s!"{path}.family: expected 'log_normal'"
        | _, _ => throw s!"{path}.family: expected 'normal' or 'log_normal'"
      let args ← expectArray s!"{path}.args" (← required path "args" fields)
      unless args.length == 2 do throw s!"{path}.args: expected exactly two strings"
      let first ← expectString s!"{path}.args[0]" (args.get! 0)
      let second ← expectString s!"{path}.args[1]" (args.get! 1)
      pure (some { family, args := (first, second) })

def parseJson (expectedDimensions : List String) (input : String) : Except String RawTable := do
  let value ← Lean.Json.parse input
  let root ← expectObject "$" value
  checkFields "$" root ["schema_version", "dimensions", "cells"]
  let version ← expectString "$.schema_version" (← required "$" "schema_version" root)
  unless version == "sembla.parameter-family/v1" ||
      version == "sembla.parameter-family/v2" do
    throw "$.schema_version: expected 'sembla.parameter-family/v1' or 'sembla.parameter-family/v2'"
  let rawDimensions ← expectArray "$.dimensions" (← required "$" "dimensions" root)
  let dimensions ← enumerate rawDimensions |>.mapM fun (item, i) => expectString s!"$.dimensions[{i}]" item
  unless dimensions == expectedDimensions do
    throw s!"$.dimensions: expected [{String.intercalate ", " expectedDimensions}]"
  let rawCells ← expectArray "$.cells" (← required "$" "cells" root)
  let cells ← enumerate rawCells |>.mapM fun (value, i) => do
    let path := s!"$.cells[{i}]"
    let fields ← expectObject path value
    checkFields path fields ["key", "default", "prior"]
    let keyFields ← expectObject s!"{path}.key" (← required path "key" fields)
    checkFields s!"{path}.key" keyFields expectedDimensions
    let key ← expectedDimensions.mapM fun dimension => do
      jsonMember s!"{path}.key.{dimension}" (← required s!"{path}.key" dimension keyFields)
    let defaultText ← expectString s!"{path}.default" (← required path "default" fields)
    let prior ← decodePrior version s!"{path}.prior" (← required path "prior" fields)
    pure { key, defaultText, prior, location := path }
  pure { dimensions, cells }

def parse (format : Format) (dimensions : List String) (input : String) : Except String RawTable :=
  match format with
  | .csv => parseCsv dimensions input
  | .csvV2 => parseCsvV2 dimensions input
  | .json => parseJson dimensions input

/-- Read, pin-check, UTF-8-decode, then parse an external table. -/
def loadPinned (format : Format) (path : System.FilePath) (dimensions : List String)
    (expectedSha256 : String) : IO (Except String RawTable) := do
  match validateSha256 expectedSha256 with
  | .error message => pure (.error message)
  | .ok () =>
      try
        let bytes ← IO.FS.readBinFile path
        let actual := Sembla.Hash.sha256Hex bytes
        if actual != expectedSha256 then
          pure (.error s!"SHA-256 mismatch: expected {expectedSha256}, found {actual}")
        else match String.fromUTF8? bytes with
          | none => pure (.error "parameter table is not valid UTF-8")
          | some text => pure (parse format dimensions text)
      catch error => pure (.error s!"unable to read parameter table '{path}': {error}")

end Sembla.ParameterTable
