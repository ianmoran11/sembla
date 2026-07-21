import Sembla.Hash

namespace Sembla.HashTests

open Sembla.Hash

#guard sha256Hex "".toUTF8 ==
  "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"

#guard sha256HexOfString "abc" ==
  "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"

#guard sha256HexOfString
    "abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq" ==
  "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"

private def hundredAs : String :=
  "aaaaaaaaaa" ++ "aaaaaaaaaa" ++ "aaaaaaaaaa" ++ "aaaaaaaaaa" ++ "aaaaaaaaaa" ++
  "aaaaaaaaaa" ++ "aaaaaaaaaa" ++ "aaaaaaaaaa" ++ "aaaaaaaaaa" ++ "aaaaaaaaaa"

#guard sha256HexOfString hundredAs ==
  "2816597888e4a0d3a36b82b83316ab32680eb8f00f8cd3b904d681246d285a0e"

#guard (domainDigest "sembla.rule-word/v1" "occ:population#infect".toUTF8).data == #[
  0x95, 0x1b, 0x64, 0xbd, 0x22, 0x2a, 0x80, 0x09,
  0xdb, 0xec, 0x87, 0xdf, 0x21, 0xc1, 0x42, 0xa5,
  0xff, 0x8e, 0x9e, 0x1b, 0xb9, 0xed, 0xa8, 0x8b,
  0x16, 0x60, 0xa4, 0xe1, 0x73, 0x9a, 0xdc, 0xd5
]

#guard hashRecord "sembla.rule-word/v1" "occ:population#infect".toUTF8 == {
  algorithm := "sha256"
  domain := "sembla.rule-word/v1"
  digest := "951b64bd222a8009dbec87df21c142a5ff8e9e1bb9eda88b1660a4e1739adcd5"
}

#guard ruleWord "occ:population#infect" == 2501600445
#guard ruleWord "occ:policy#restrict" == 2426532370
#guard ruleWord "occ:north/population#infect" == 1000729077
#guard ruleWord "occ:south/population#infect" == 3844621682
#guard ruleWord "occ:epidemic/population#infect" == 3914077761

#guard !isReservedRuleWord 0xFFFFFFFD
#guard isReservedRuleWord 0xFFFFFFFE
#guard isReservedRuleWord 0xFFFFFFFF

end Sembla.HashTests
