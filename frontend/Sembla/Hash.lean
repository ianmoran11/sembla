import Std

namespace Sembla.Hash

/-- SHA-256 round constants from FIPS 180-4 §4.2.2. -/
private def roundConstants : Array UInt32 := #[
  0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5,
  0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
  0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
  0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
  0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc,
  0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
  0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
  0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
  0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
  0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
  0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3,
  0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
  0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5,
  0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
  0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
  0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2
]

/-- Initial SHA-256 hash values from FIPS 180-4 §5.3.3. -/
private def initialHash : Array UInt32 := #[
  0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
  0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19
]

private def rotateRight (x n : UInt32) : UInt32 :=
  (x >>> n) ||| (x <<< (32 - n))

private def choose (x y z : UInt32) : UInt32 :=
  (x &&& y) ^^^ ((~~~x) &&& z)

private def majority (x y z : UInt32) : UInt32 :=
  (x &&& y) ^^^ (x &&& z) ^^^ (y &&& z)

private def bigSigma0 (x : UInt32) : UInt32 :=
  rotateRight x 2 ^^^ rotateRight x 13 ^^^ rotateRight x 22

private def bigSigma1 (x : UInt32) : UInt32 :=
  rotateRight x 6 ^^^ rotateRight x 11 ^^^ rotateRight x 25

private def smallSigma0 (x : UInt32) : UInt32 :=
  rotateRight x 7 ^^^ rotateRight x 18 ^^^ (x >>> 3)

private def smallSigma1 (x : UInt32) : UInt32 :=
  rotateRight x 17 ^^^ rotateRight x 19 ^^^ (x >>> 10)

private def readUInt32BE (bytes : ByteArray) (offset : Nat) : UInt32 :=
  (bytes.get! offset).toUInt32 <<< 24 |||
  (bytes.get! (offset + 1)).toUInt32 <<< 16 |||
  (bytes.get! (offset + 2)).toUInt32 <<< 8 |||
  (bytes.get! (offset + 3)).toUInt32

private def appendUInt64BE (bytes : ByteArray) (word : UInt64) : ByteArray := Id.run do
  let mut out := bytes
  for i in [:8] do
    let shift := UInt64.ofNat ((7 - i) * 8)
    out := out.push ((word >>> shift).toUInt8)
  return out

private def padMessage (data : ByteArray) : ByteArray := Id.run do
  let mut padded := data.push 0x80
  while padded.size % 64 != 56 do
    padded := padded.push 0
  let bitLength := UInt64.ofNat data.size * 8
  return appendUInt64BE padded bitLength

private def messageSchedule (padded : ByteArray) (blockOffset : Nat) : Array UInt32 := Id.run do
  let mut words := mkArray 64 (0 : UInt32)
  for i in [:16] do
    words := words.set! i (readUInt32BE padded (blockOffset + i * 4))
  for i in [16:64] do
    let word := smallSigma1 words[i - 2]! + words[i - 7]! +
      smallSigma0 words[i - 15]! + words[i - 16]!
    words := words.set! i word
  return words

/-- Compute the 32-byte FIPS 180-4 SHA-256 digest of `data`. -/
def sha256 (data : ByteArray) : ByteArray := Id.run do
  let padded := padMessage data
  let mut state := initialHash
  for block in [:padded.size / 64] do
    let words := messageSchedule padded (block * 64)
    let mut a := state[0]!
    let mut b := state[1]!
    let mut c := state[2]!
    let mut d := state[3]!
    let mut e := state[4]!
    let mut f := state[5]!
    let mut g := state[6]!
    let mut h := state[7]!
    for i in [:64] do
      let temp1 := h + bigSigma1 e + choose e f g + roundConstants[i]! + words[i]!
      let temp2 := bigSigma0 a + majority a b c
      h := g
      g := f
      f := e
      e := d + temp1
      d := c
      c := b
      b := a
      a := temp1 + temp2
    state := state.set! 0 (state[0]! + a)
    state := state.set! 1 (state[1]! + b)
    state := state.set! 2 (state[2]! + c)
    state := state.set! 3 (state[3]! + d)
    state := state.set! 4 (state[4]! + e)
    state := state.set! 5 (state[5]! + f)
    state := state.set! 6 (state[6]! + g)
    state := state.set! 7 (state[7]! + h)
  let mut digest := ByteArray.empty
  for i in [:8] do
    let word := state[i]!
    digest := digest.push (word >>> 24).toUInt8
    digest := digest.push (word >>> 16).toUInt8
    digest := digest.push (word >>> 8).toUInt8
    digest := digest.push word.toUInt8
  return digest

private def hexDigit (n : Nat) : Char :=
  if n < 10 then Char.ofNat ('0'.toNat + n)
  else Char.ofNat ('a'.toNat + n - 10)

/-- Compute the lowercase, 64-character hexadecimal SHA-256 digest of `data`. -/
def sha256Hex (data : ByteArray) : String := Id.run do
  let digest := sha256 data
  let mut out := ""
  for i in [:digest.size] do
    let byte := (digest.get! i).toNat
    out := out.push (hexDigit (byte / 16))
    out := out.push (hexDigit (byte % 16))
  return out

/-- Compute the lowercase SHA-256 digest of the UTF-8 bytes of `s`. -/
def sha256HexOfString (s : String) : String :=
  sha256Hex s.toUTF8

/-- A domain-tagged SHA-256 digest record. -/
structure HashRecord where
  algorithm : String
  domain : String
  digest : String
  deriving Repr, BEq

/--
SHA-256(`domain ++ 0x00 ++ payload`). The single zero separator is the frozen
hash-domain convention from `DECISIONS.md` §J2.
-/
def domainDigest (domain : String) (payload : ByteArray) : ByteArray :=
  sha256 ((domain.toUTF8.push 0).append payload)

/-- Build a SHA-256 record for a domain-separated payload. -/
def hashRecord (domain : String) (payload : ByteArray) : HashRecord :=
  { algorithm := "sha256"
    domain
    digest := sha256Hex ((domain.toUTF8.push 0).append payload) }

/--
Derive the big-endian `UInt32` in the first four bytes of the frozen
`sembla.rule-word/v1` domain digest for `identity`.
-/
def ruleWord (identity : String) : UInt32 :=
  readUInt32BE (domainDigest "sembla.rule-word/v1" identity.toUTF8) 0

/-- Whether `w` is one of the runtime's reserved sweep/prior rule words. -/
def isReservedRuleWord (w : UInt32) : Bool :=
  w == 0xFFFFFFFE || w == 0xFFFFFFFF

end Sembla.Hash
