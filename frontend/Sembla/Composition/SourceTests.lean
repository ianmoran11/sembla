import Sembla.Composition.Fixtures

namespace Sembla.Composition.SourceTests

open Sembla
open Sembla.Composition.Fixtures

private def roundTrips (source : CompositionSourceV1) : Bool :=
  let bytes := Json.render source
  match Json.parse bytes with
  | .error _ => false
  | .ok parsed => Json.render parsed == bytes

#guard Fixtures.corpus.map (·.1) == [
  "solo_population", "independent_epidemic_policy", "two_independent_regions",
  "epidemic_policy", "ping_pong", "wrapped_ping_pong", "two_regions", "regional_response"]
#guard Fixtures.corpus.all fun entry => roundTrips entry.2
private def fixtureIsWellFormed (source : CompositionSourceV1) : Bool :=
  match wellFormed source with
  | .ok () => true
  | .error _ => false

#guard Fixtures.corpus.all fun entry => fixtureIsWellFormed entry.2
#guard fixtureIsWellFormed twoRegionsDisplayRenamed

/-- Independently frozen non-canonical input.  Object keys are deliberately
    reversed at every level and whitespace is hand-authored in this literal;
    this test input must not be derived from `Json.encode` or `Json.render`. -/
private def prettyEpidemicPolicy : String :=
  "{\n" ++
  "  \"summaries\": [],\n" ++
  "  \"schema_version\": \"sembla.composition-source/v1\",\n" ++
  "  \"root_definition\": \"def:epidemic_policy\",\n" ++
  "  \"required_features\": [],\n" ++
  "  \"parameters\": [\n" ++
  "    {\n" ++
  "      \"ty\": \"real\",\n" ++
  "      \"prior\": {\n" ++
  "        \"family\": \"log_normal\",\n" ++
  "        \"args\": [\n" ++
  "          -0.2231435513142097,\n" ++
  "          0.25\n" ++
  "        ]\n" ++
  "      },\n" ++
  "      \"name\": \"beta\",\n" ++
  "      \"default\": {\n" ++
  "        \"value\": 0.8,\n" ++
  "        \"kind\": \"real\"\n" ++
  "      }\n" ++
  "    },\n" ++
  "    {\n" ++
  "      \"ty\": \"real\",\n" ++
  "      \"prior\": {\n" ++
  "        \"family\": \"log_normal\",\n" ++
  "        \"args\": [\n" ++
  "          -2.302585092994046,\n" ++
  "          0.25\n" ++
  "        ]\n" ++
  "      },\n" ++
  "      \"name\": \"gamma\",\n" ++
  "      \"default\": {\n" ++
  "        \"value\": 0.1,\n" ++
  "        \"kind\": \"real\"\n" ++
  "      }\n" ++
  "    }\n" ++
  "  ],\n" ++
  "  \"outer_dt\": 0.25,\n" ++
  "  \"model_id\": \"model:epidemic_policy\",\n" ++
  "  \"display_name\": \"Epidemic policy\",\n" ++
  "  \"definitions\": [\n" ++
  "    {\n" ++
  "      \"views\": [\n" ++
  "        {\n" ++
  "          \"value\": null,\n" ++
  "          \"table\": \"person\",\n" ++
  "          \"reduce\": \"count\",\n" ++
  "          \"name\": \"S\",\n" ++
  "          \"filter\": {\n" ++
  "            \"variant\": \"S\",\n" ++
  "            \"kind\": \"enum_is\",\n" ++
  "            \"attr\": \"health\"\n" ++
  "          }\n" ++
  "        },\n" ++
  "        {\n" ++
  "          \"value\": null,\n" ++
  "          \"table\": \"person\",\n" ++
  "          \"reduce\": \"count\",\n" ++
  "          \"name\": \"I\",\n" ++
  "          \"filter\": {\n" ++
  "            \"variant\": \"I\",\n" ++
  "            \"kind\": \"enum_is\",\n" ++
  "            \"attr\": \"health\"\n" ++
  "          }\n" ++
  "        },\n" ++
  "        {\n" ++
  "          \"value\": null,\n" ++
  "          \"table\": \"person\",\n" ++
  "          \"reduce\": \"count\",\n" ++
  "          \"name\": \"R\",\n" ++
  "          \"filter\": {\n" ++
  "            \"variant\": \"R\",\n" ++
  "            \"kind\": \"enum_is\",\n" ++
  "            \"attr\": \"health\"\n" ++
  "          }\n" ++
  "        }\n" ++
  "      ],\n" ++
  "      \"transitions\": [\n" ++
  "        {\n" ++
  "          \"table\": \"person\",\n" ++
  "          \"name\": \"infect\",\n" ++
  "          \"hazard\": {\n" ++
  "            \"rhs\": {\n" ++
  "              \"rhs\": {\n" ++
  "                \"port\": \"restriction_modifier\",\n" ++
  "                \"kind\": \"input\",\n" ++
  "                \"agg\": {\n" ++
  "                  \"op\": {\n" ++
  "                    \"value\": {\n" ++
  "                      \"name\": \"restriction\",\n" ++
  "                      \"kind\": \"self_attr\"\n" ++
  "                    },\n" ++
  "                    \"kind\": \"sum\"\n" ++
  "                  },\n" ++
  "                  \"filter\": null\n" ++
  "                }\n" ++
  "              },\n" ++
  "              \"lhs\": {\n" ++
  "                \"value\": 1.0,\n" ++
  "                \"kind\": \"real\"\n" ++
  "              },\n" ++
  "              \"kind\": \"add\"\n" ++
  "            },\n" ++
  "            \"lhs\": {\n" ++
  "              \"rhs\": {\n" ++
  "                \"rhs\": {\n" ++
  "                  \"table\": \"person\",\n" ++
  "                  \"op\": {\n" ++
  "                    \"kind\": \"count\"\n" ++
  "                  },\n" ++
  "                  \"on\": {\n" ++
  "                    \"self_fk_attr\": \"employer\",\n" ++
  "                    \"fk_attr\": \"employer\"\n" ++
  "                  },\n" ++
  "                  \"kind\": \"agg\",\n" ++
  "                  \"filter\": {\n" ++
  "                    \"value\": true,\n" ++
  "                    \"kind\": \"bool\"\n" ++
  "                  }\n" ++
  "                },\n" ++
  "                \"lhs\": {\n" ++
  "                  \"table\": \"person\",\n" ++
  "                  \"op\": {\n" ++
  "                    \"kind\": \"count\"\n" ++
  "                  },\n" ++
  "                  \"on\": {\n" ++
  "                    \"self_fk_attr\": \"employer\",\n" ++
  "                    \"fk_attr\": \"employer\"\n" ++
  "                  },\n" ++
  "                  \"kind\": \"agg\",\n" ++
  "                  \"filter\": {\n" ++
  "                    \"variant\": \"I\",\n" ++
  "                    \"kind\": \"enum_is\",\n" ++
  "                    \"attr\": \"health\"\n" ++
  "                  }\n" ++
  "                },\n" ++
  "                \"kind\": \"div\"\n" ++
  "              },\n" ++
  "              \"lhs\": {\n" ++
  "                \"name\": \"beta\",\n" ++
  "                \"kind\": \"param\"\n" ++
  "              },\n" ++
  "              \"kind\": \"mul\"\n" ++
  "            },\n" ++
  "            \"kind\": \"mul\"\n" ++
  "          },\n" ++
  "          \"guard\": {\n" ++
  "            \"variant\": \"S\",\n" ++
  "            \"kind\": \"enum_is\",\n" ++
  "            \"attr\": \"health\"\n" ++
  "          },\n" ++
  "          \"effects\": [\n" ++
  "            {\n" ++
  "              \"value\": {\n" ++
  "                \"variant\": \"I\",\n" ++
  "                \"kind\": \"enum\"\n" ++
  "              },\n" ++
  "              \"kind\": \"set_attr\",\n" ++
  "              \"attr\": \"health\"\n" ++
  "            }\n" ++
  "          ],\n" ++
  "          \"contests\": []\n" ++
  "        },\n" ++
  "        {\n" ++
  "          \"table\": \"person\",\n" ++
  "          \"name\": \"recover\",\n" ++
  "          \"hazard\": {\n" ++
  "            \"name\": \"gamma\",\n" ++
  "            \"kind\": \"param\"\n" ++
  "          },\n" ++
  "          \"guard\": {\n" ++
  "            \"variant\": \"I\",\n" ++
  "            \"kind\": \"enum_is\",\n" ++
  "            \"attr\": \"health\"\n" ++
  "          },\n" ++
  "          \"effects\": [\n" ++
  "            {\n" ++
  "              \"value\": {\n" ++
  "                \"variant\": \"R\",\n" ++
  "                \"kind\": \"enum\"\n" ++
  "              },\n" ++
  "              \"kind\": \"set_attr\",\n" ++
  "              \"attr\": \"health\"\n" ++
  "            }\n" ++
  "          ],\n" ++
  "          \"contests\": []\n" ++
  "        }\n" ++
  "      ],\n" ++
  "      \"tables\": [\n" ++
  "        {\n" ++
  "          \"size_hint\": 1000,\n" ++
  "          \"name\": \"person\",\n" ++
  "          \"attrs\": [\n" ++
  "            {\n" ++
  "              \"ty\": {\n" ++
  "                \"variants\": [\n" ++
  "                  \"S\",\n" ++
  "                  \"I\",\n" ++
  "                  \"R\"\n" ++
  "                ],\n" ++
  "                \"kind\": \"enum\"\n" ++
  "              },\n" ++
  "              \"name\": \"health\"\n" ++
  "            },\n" ++
  "            {\n" ++
  "              \"ty\": {\n" ++
  "                \"table\": \"employer\",\n" ++
  "                \"kind\": \"ref\"\n" ++
  "              },\n" ++
  "              \"name\": \"employer\"\n" ++
  "            }\n" ++
  "          ]\n" ++
  "        },\n" ++
  "        {\n" ++
  "          \"size_hint\": 50,\n" ++
  "          \"name\": \"employer\",\n" ++
  "          \"attrs\": []\n" ++
  "        }\n" ++
  "      ],\n" ++
  "      \"ports\": [\n" ++
  "        {\n" ++
  "          \"schema\": [\n" ++
  "            {\n" ++
  "              \"ty\": {\n" ++
  "                \"kind\": \"real\"\n" ++
  "              },\n" ++
  "              \"name\": \"restriction\"\n" ++
  "            }\n" ++
  "          ],\n" ++
  "          \"id\": \"port:restriction_modifier\",\n" ++
  "          \"display_name\": \"Restriction modifier\",\n" ++
  "          \"direction\": \"input\"\n" ++
  "        },\n" ++
  "        {\n" ++
  "          \"schema\": [\n" ++
  "            {\n" ++
  "              \"ty\": {\n" ++
  "                \"kind\": \"int\"\n" ++
  "              },\n" ++
  "              \"name\": \"infected\"\n" ++
  "            }\n" ++
  "          ],\n" ++
  "          \"id\": \"port:infection_count\",\n" ++
  "          \"display_name\": \"Infection count\",\n" ++
  "          \"direction\": \"output\"\n" ++
  "        }\n" ++
  "      ],\n" ++
  "      \"parameter_requirements\": [\n" ++
  "        \"beta\",\n" ++
  "        \"gamma\"\n" ++
  "      ],\n" ++
  "      \"outputs\": [\n" ++
  "        {\n" ++
  "          \"schema\": [\n" ++
  "            {\n" ++
  "              \"ty\": {\n" ++
  "                \"kind\": \"int\"\n" ++
  "              },\n" ++
  "              \"name\": \"infected\"\n" ++
  "            }\n" ++
  "          ],\n" ++
  "          \"name\": \"infection_count\",\n" ++
  "          \"builder\": {\n" ++
  "            \"table\": \"person\",\n" ++
  "            \"kind\": \"per_table\",\n" ++
  "            \"fields\": [\n" ++
  "              {\n" ++
  "                \"op\": {\n" ++
  "                  \"kind\": \"count\"\n" ++
  "                },\n" ++
  "                \"name\": \"infected\",\n" ++
  "                \"filter\": {\n" ++
  "                  \"variant\": \"I\",\n" ++
  "                  \"kind\": \"enum_is\",\n" ++
  "                  \"attr\": \"health\"\n" ++
  "                }\n" ++
  "              }\n" ++
  "            ]\n" ++
  "          }\n" ++
  "        }\n" ++
  "      ],\n" ++
  "      \"kind\": \"primitive\",\n" ++
  "      \"inputs\": [\n" ++
  "        {\n" ++
  "          \"schema\": [\n" ++
  "            {\n" ++
  "              \"ty\": {\n" ++
  "                \"kind\": \"real\"\n" ++
  "              },\n" ++
  "              \"name\": \"restriction\"\n" ++
  "            }\n" ++
  "          ],\n" ++
  "          \"name\": \"restriction_modifier\"\n" ++
  "        }\n" ++
  "      ],\n" ++
  "      \"id\": \"def:population\",\n" ++
  "      \"display_name\": \"Population\"\n" ++
  "    },\n" ++
  "    {\n" ++
  "      \"views\": [],\n" ++
  "      \"transitions\": [\n" ++
  "        {\n" ++
  "          \"table\": \"controller\",\n" ++
  "          \"name\": \"restrict\",\n" ++
  "          \"hazard\": {\n" ++
  "            \"value\": 1e+300,\n" ++
  "            \"kind\": \"real\"\n" ++
  "          },\n" ++
  "          \"guard\": {\n" ++
  "            \"rhs\": {\n" ++
  "              \"rhs\": {\n" ++
  "                \"value\": 500,\n" ++
  "                \"kind\": \"int\"\n" ++
  "              },\n" ++
  "              \"lhs\": {\n" ++
  "                \"port\": \"infection_count\",\n" ++
  "                \"kind\": \"input\",\n" ++
  "                \"agg\": {\n" ++
  "                  \"op\": {\n" ++
  "                    \"value\": {\n" ++
  "                      \"name\": \"infected\",\n" ++
  "                      \"kind\": \"self_attr\"\n" ++
  "                    },\n" ++
  "                    \"kind\": \"sum\"\n" ++
  "                  },\n" ++
  "                  \"filter\": null\n" ++
  "                }\n" ++
  "              },\n" ++
  "              \"kind\": \"gt\"\n" ++
  "            },\n" ++
  "            \"lhs\": {\n" ++
  "              \"variant\": \"Open\",\n" ++
  "              \"kind\": \"enum_is\",\n" ++
  "              \"attr\": \"mode\"\n" ++
  "            },\n" ++
  "            \"kind\": \"and\"\n" ++
  "          },\n" ++
  "          \"effects\": [\n" ++
  "            {\n" ++
  "              \"value\": {\n" ++
  "                \"variant\": \"Restricted\",\n" ++
  "                \"kind\": \"enum\"\n" ++
  "              },\n" ++
  "              \"kind\": \"set_attr\",\n" ++
  "              \"attr\": \"mode\"\n" ++
  "            },\n" ++
  "            {\n" ++
  "              \"value\": {\n" ++
  "                \"value\": 0.4,\n" ++
  "                \"kind\": \"real\"\n" ++
  "              },\n" ++
  "              \"kind\": \"set_attr\",\n" ++
  "              \"attr\": \"modifier\"\n" ++
  "            }\n" ++
  "          ],\n" ++
  "          \"contests\": []\n" ++
  "        },\n" ++
  "        {\n" ++
  "          \"table\": \"controller\",\n" ++
  "          \"name\": \"reopen\",\n" ++
  "          \"hazard\": {\n" ++
  "            \"value\": 1e+300,\n" ++
  "            \"kind\": \"real\"\n" ++
  "          },\n" ++
  "          \"guard\": {\n" ++
  "            \"rhs\": {\n" ++
  "              \"rhs\": {\n" ++
  "                \"value\": 150,\n" ++
  "                \"kind\": \"int\"\n" ++
  "              },\n" ++
  "              \"lhs\": {\n" ++
  "                \"port\": \"infection_count\",\n" ++
  "                \"kind\": \"input\",\n" ++
  "                \"agg\": {\n" ++
  "                  \"op\": {\n" ++
  "                    \"value\": {\n" ++
  "                      \"name\": \"infected\",\n" ++
  "                      \"kind\": \"self_attr\"\n" ++
  "                    },\n" ++
  "                    \"kind\": \"sum\"\n" ++
  "                  },\n" ++
  "                  \"filter\": null\n" ++
  "                }\n" ++
  "              },\n" ++
  "              \"kind\": \"lt\"\n" ++
  "            },\n" ++
  "            \"lhs\": {\n" ++
  "              \"variant\": \"Restricted\",\n" ++
  "              \"kind\": \"enum_is\",\n" ++
  "              \"attr\": \"mode\"\n" ++
  "            },\n" ++
  "            \"kind\": \"and\"\n" ++
  "          },\n" ++
  "          \"effects\": [\n" ++
  "            {\n" ++
  "              \"value\": {\n" ++
  "                \"variant\": \"Open\",\n" ++
  "                \"kind\": \"enum\"\n" ++
  "              },\n" ++
  "              \"kind\": \"set_attr\",\n" ++
  "              \"attr\": \"mode\"\n" ++
  "            },\n" ++
  "            {\n" ++
  "              \"value\": {\n" ++
  "                \"value\": 1.0,\n" ++
  "                \"kind\": \"real\"\n" ++
  "              },\n" ++
  "              \"kind\": \"set_attr\",\n" ++
  "              \"attr\": \"modifier\"\n" ++
  "            }\n" ++
  "          ],\n" ++
  "          \"contests\": []\n" ++
  "        }\n" ++
  "      ],\n" ++
  "      \"tables\": [\n" ++
  "        {\n" ++
  "          \"size_hint\": 1,\n" ++
  "          \"name\": \"controller\",\n" ++
  "          \"attrs\": [\n" ++
  "            {\n" ++
  "              \"ty\": {\n" ++
  "                \"variants\": [\n" ++
  "                  \"Open\",\n" ++
  "                  \"Restricted\"\n" ++
  "                ],\n" ++
  "                \"kind\": \"enum\"\n" ++
  "              },\n" ++
  "              \"name\": \"mode\"\n" ++
  "            },\n" ++
  "            {\n" ++
  "              \"ty\": {\n" ++
  "                \"kind\": \"real\"\n" ++
  "              },\n" ++
  "              \"name\": \"modifier\"\n" ++
  "            }\n" ++
  "          ]\n" ++
  "        }\n" ++
  "      ],\n" ++
  "      \"ports\": [\n" ++
  "        {\n" ++
  "          \"schema\": [\n" ++
  "            {\n" ++
  "              \"ty\": {\n" ++
  "                \"kind\": \"int\"\n" ++
  "              },\n" ++
  "              \"name\": \"infected\"\n" ++
  "            }\n" ++
  "          ],\n" ++
  "          \"id\": \"port:infection_count\",\n" ++
  "          \"display_name\": \"Infection count\",\n" ++
  "          \"direction\": \"input\"\n" ++
  "        },\n" ++
  "        {\n" ++
  "          \"schema\": [\n" ++
  "            {\n" ++
  "              \"ty\": {\n" ++
  "                \"kind\": \"real\"\n" ++
  "              },\n" ++
  "              \"name\": \"restriction\"\n" ++
  "            }\n" ++
  "          ],\n" ++
  "          \"id\": \"port:restriction_modifier\",\n" ++
  "          \"display_name\": \"Restriction modifier\",\n" ++
  "          \"direction\": \"output\"\n" ++
  "        }\n" ++
  "      ],\n" ++
  "      \"parameter_requirements\": [],\n" ++
  "      \"outputs\": [\n" ++
  "        {\n" ++
  "          \"schema\": [\n" ++
  "            {\n" ++
  "              \"ty\": {\n" ++
  "                \"kind\": \"real\"\n" ++
  "              },\n" ++
  "              \"name\": \"restriction\"\n" ++
  "            }\n" ++
  "          ],\n" ++
  "          \"name\": \"restriction_modifier\",\n" ++
  "          \"builder\": {\n" ++
  "            \"table\": \"controller\",\n" ++
  "            \"kind\": \"per_table\",\n" ++
  "            \"fields\": [\n" ++
  "              {\n" ++
  "                \"op\": {\n" ++
  "                  \"value\": {\n" ++
  "                    \"rhs\": {\n" ++
  "                      \"value\": 1.0,\n" ++
  "                      \"kind\": \"real\"\n" ++
  "                    },\n" ++
  "                    \"lhs\": {\n" ++
  "                      \"name\": \"modifier\",\n" ++
  "                      \"kind\": \"self_attr\"\n" ++
  "                    },\n" ++
  "                    \"kind\": \"sub\"\n" ++
  "                  },\n" ++
  "                  \"kind\": \"sum\"\n" ++
  "                },\n" ++
  "                \"name\": \"restriction\",\n" ++
  "                \"filter\": null\n" ++
  "              }\n" ++
  "            ]\n" ++
  "          }\n" ++
  "        }\n" ++
  "      ],\n" ++
  "      \"kind\": \"primitive\",\n" ++
  "      \"inputs\": [\n" ++
  "        {\n" ++
  "          \"schema\": [\n" ++
  "            {\n" ++
  "              \"ty\": {\n" ++
  "                \"kind\": \"int\"\n" ++
  "              },\n" ++
  "              \"name\": \"infected\"\n" ++
  "            }\n" ++
  "          ],\n" ++
  "          \"name\": \"infection_count\"\n" ++
  "        }\n" ++
  "      ],\n" ++
  "      \"id\": \"def:policy\",\n" ++
  "      \"display_name\": \"Policy\"\n" ++
  "    },\n" ++
  "    {\n" ++
  "      \"wires\": [\n" ++
  "        {\n" ++
  "          \"target_port\": \"port:infection_count\",\n" ++
  "          \"target_instance\": \"inst:policy\",\n" ++
  "          \"source_port\": \"port:infection_count\",\n" ++
  "          \"source_instance\": \"inst:population\",\n" ++
  "          \"id\": \"wire:count_to_policy\",\n" ++
  "          \"delay_ticks\": 1\n" ++
  "        },\n" ++
  "        {\n" ++
  "          \"target_port\": \"port:restriction_modifier\",\n" ++
  "          \"target_instance\": \"inst:population\",\n" ++
  "          \"source_port\": \"port:restriction_modifier\",\n" ++
  "          \"source_instance\": \"inst:policy\",\n" ++
  "          \"id\": \"wire:restriction_to_population\",\n" ++
  "          \"delay_ticks\": 1\n" ++
  "        }\n" ++
  "      ],\n" ++
  "      \"ports\": [],\n" ++
  "      \"parameter_requirements\": [\n" ++
  "        \"beta\",\n" ++
  "        \"gamma\"\n" ++
  "      ],\n" ++
  "      \"kind\": \"composite\",\n" ++
  "      \"instances\": [\n" ++
  "        {\n" ++
  "          \"parameter_bindings\": [\n" ++
  "            {\n" ++
  "              \"requirement\": \"beta\",\n" ++
  "              \"parameter\": \"beta\"\n" ++
  "            },\n" ++
  "            {\n" ++
  "              \"requirement\": \"gamma\",\n" ++
  "              \"parameter\": \"gamma\"\n" ++
  "            }\n" ++
  "          ],\n" ++
  "          \"id\": \"inst:population\",\n" ++
  "          \"display_name\": \"Population\",\n" ++
  "          \"definition\": \"def:population\"\n" ++
  "        },\n" ++
  "        {\n" ++
  "          \"parameter_bindings\": [],\n" ++
  "          \"id\": \"inst:policy\",\n" ++
  "          \"display_name\": \"Policy\",\n" ++
  "          \"definition\": \"def:policy\"\n" ++
  "        }\n" ++
  "      ],\n" ++
  "      \"id\": \"def:epidemic_policy\",\n" ++
  "      \"hidden_ports\": [],\n" ++
  "      \"exposures\": [],\n" ++
  "      \"display_name\": \"Epidemic policy\"\n" ++
  "    }\n" ++
  "  ]\n" ++
  "}\n"

private def reformattedEpidemicPolicyCanonicalizes : Bool :=
  let canonical := Json.render epidemicPolicy
  prettyEpidemicPolicy != canonical &&
    match Json.parse prettyEpidemicPolicy with
    | .error _ => false
    | .ok parsed => Json.render parsed == canonical

#guard reformattedEpidemicPolicyCanonicalizes

private def modifyAt (index : Nat) (transform : α → α) : List α → List α
  | [] => []
  | value :: rest =>
      if index == 0 then transform value :: rest
      else value :: modifyAt (index - 1) transform rest

private def modifyDefinition
    (source : CompositionSourceV1) (index : Nat)
    (transform : ComponentDefinitionV1 → ComponentDefinitionV1) : CompositionSourceV1 :=
  { source with definitions := modifyAt index transform source.definitions }

private def parseErrorEq (bytes expected : String) : Bool :=
  match Json.parse bytes with
  | .error message => message == expected
  | .ok _ => false

private def badSlugSource : CompositionSourceV1 :=
  modifyDefinition soloPopulation 0 fun definition =>
    { definition with id := ⟨"def:North"⟩ }

#guard parseErrorEq (Json.render badSlugSource)
  "definitions[0].id: payload 'North' is not a slug"

private def wrongDefinitionKindSource : CompositionSourceV1 :=
  modifyDefinition soloPopulation 0 fun definition =>
    { definition with id := ⟨"inst:population"⟩ }

#guard parseErrorEq (Json.render wrongDefinitionKindSource)
  "definitions[0].id: expected 'def:' stable id; got 'inst:population'"

private def delayTwoSource : CompositionSourceV1 :=
  modifyDefinition epidemicPolicy 2 fun definition =>
    match definition.body with
    | .primitive _ => definition
    | .composite body =>
        { definition with body := .composite {
            body with «wires» := (modifyAt 0 (fun item => { item with delayTicks := 2 }) body.wires) } }

#guard parseErrorEq (Json.render delayTwoSource)
  "definitions[2].wires[0].delay_ticks: V1 requires exactly 1"

private def duplicateDefinitionSource : CompositionSourceV1 :=
  modifyDefinition soloPopulation 1 fun definition =>
    { definition with id := ⟨"def:population"⟩ }

#guard parseErrorEq (Json.render duplicateDefinitionSource)
  "definitions[1].id: duplicate definition id 'def:population'"

private def duplicateInstanceSource : CompositionSourceV1 :=
  modifyDefinition epidemicPolicy 2 fun definition =>
    match definition.body with
    | .primitive _ => definition
    | .composite body =>
        { definition with body := .composite {
            body with instances := (modifyAt 1
              (fun item => { item with id := ⟨"inst:population"⟩ }) body.instances) } }

#guard parseErrorEq (Json.render duplicateInstanceSource)
  "definitions[2].instances[1].id: duplicate instance id 'inst:population'"

private def requiredFeatureSource : CompositionSourceV1 :=
  { epidemicPolicy with requiredFeatures := ["future_feature"] }

#guard parseErrorEq (Json.render requiredFeatureSource)
  "required_features[0]: unsupported feature 'future_feature'"

private def addTopUnknown : PlanJson.CJson → PlanJson.CJson
  | .obj entries => .obj (entries.push ("foo", .int 0))
  | value => value

#guard parseErrorEq (addTopUnknown (Json.encode epidemicPolicy)).render
  "unknown field 'foo' at $"

private def addObjectField (name : String) (value : PlanJson.CJson) : PlanJson.CJson → PlanJson.CJson
  | .obj entries => .obj (entries.push (name, value))
  | other => other

private def addFirstDefinitionUnknown : PlanJson.CJson → PlanJson.CJson
  | .obj entries => .obj (entries.map fun entry =>
      if entry.1 == "definitions" then
        match entry.2 with
        | .arr items =>
            match items.toList with
            | [] => entry
            | first :: rest =>
                (entry.1, .arr ((addObjectField "foo" (.int 0) first :: rest).toArray))
        | _ => entry
      else entry)
  | value => value

#guard parseErrorEq (addFirstDefinitionUnknown (Json.encode epidemicPolicy)).render
  "unknown field 'foo' at definitions[0]"

/- DECISIONS §K6 keeps grouped primitive bodies out of composition sources. -/
private def addFirstDefinitionGroupedViews : PlanJson.CJson → PlanJson.CJson
  | .obj entries => .obj (entries.map fun entry =>
      if entry.1 == "definitions" then
        match entry.2 with
        | .arr items =>
            match items.toList with
            | [] => entry
            | first :: rest =>
                (entry.1, .arr ((addObjectField "grouped_views" (.arr #[]) first :: rest).toArray))
        | _ => entry
      else entry)
  | value => value

#guard parseErrorEq (addFirstDefinitionGroupedViews (Json.encode epidemicPolicy)).render
  "definitions[0].grouped_views: grouped views are not yet supported in composition sources (DECISIONS §K6)"

private def unknownSchemaSource : CompositionSourceV1 :=
  { epidemicPolicy with schemaVersion := "sembla.composition-source/v2" }

#guard parseErrorEq (Json.render unknownSchemaSource)
  "unknown schema_version 'sembla.composition-source/v2'; supported: sembla.composition-source/v1"

private def unmatchedOutputPortSource : CompositionSourceV1 :=
  modifyDefinition epidemicPolicy 0 fun definition =>
    { definition with ports := (modifyAt 1
        (fun port => { port with id := ⟨"port:missing"⟩ }) definition.ports) }

#guard parseErrorEq (Json.render unmatchedOutputPortSource)
  "definitions[0].ports[1]: port 'port:missing' has no matching primitive output"

private def mismatchedOutputSchemaSource : CompositionSourceV1 :=
  modifyDefinition epidemicPolicy 0 fun definition =>
    { definition with ports := (modifyAt 1
        (fun port => { port with schema := [] }) definition.ports) }

#guard parseErrorEq (Json.render mismatchedOutputSchemaSource)
  "definitions[0].ports[1].schema: does not match primitive output 'infection_count' schema"

end Sembla.Composition.SourceTests
