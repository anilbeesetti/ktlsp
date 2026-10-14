# Frozen preimplementation expectations

The JSON block below is byte-identical to the independently reviewed contract
EXPECTATIONS.json; no decoder/compiler/native execution produced these values.
All nine valid literals,127 full-prefix outcomes and15 malformed outcomes are
preserved. Argument.raw and InlineType.raw/spans retain their declared frames.

```json
{
  "stage": "independently authored CONTRACT; UNEXECUTED",
  "all_other_header_scalars": "None",
  "prefix_success": "only complete top-level fields retained; arguments/header inherit those fields, no defaults applied",
  "valid": [
    {
      "name": "selected",
      "hex": "08 01 12 08 08 01 12 02 38 00 18 7f 30 02",
      "N": 14,
      "F": 3,
      "K": 1,
      "M": [
        8
      ],
      "G": [
        3
      ],
      "j": [
        1
      ],
      "header": {
        "flags": 1,
        "class_name": 2
      },
      "arguments": [
        {
          "span": [
            4,
            12
          ],
          "projection": "Out",
          "type_id": 127,
          "inline": [
            {
              "span": [
                4,
                6
              ],
              "hex": "38 00"
            }
          ],
          "raw_hex": "08 01 12 02 38 00 18 7f"
        }
      ],
      "success_prefixes": [
        0,
        2,
        12,
        14
      ],
      "minimum": {
        "records": 9,
        "nodes": 2,
        "depth": 2,
        "owned_A": 1,
        "owned_S": 1,
        "work_constant": 56
      },
      "prefix_outcomes": [
        {
          "length": 0,
          "result": "success",
          "offset": null
        },
        {
          "length": 1,
          "result": "Truncated",
          "offset": 1
        },
        {
          "length": 2,
          "result": "success",
          "offset": null
        },
        {
          "length": 3,
          "result": "Truncated",
          "offset": 3
        },
        {
          "length": 4,
          "result": "Truncated",
          "offset": 4
        },
        {
          "length": 5,
          "result": "Truncated",
          "offset": 5
        },
        {
          "length": 6,
          "result": "Truncated",
          "offset": 6
        },
        {
          "length": 7,
          "result": "Truncated",
          "offset": 7
        },
        {
          "length": 8,
          "result": "Truncated",
          "offset": 8
        },
        {
          "length": 9,
          "result": "Truncated",
          "offset": 9
        },
        {
          "length": 10,
          "result": "Truncated",
          "offset": 10
        },
        {
          "length": 11,
          "result": "Truncated",
          "offset": 11
        },
        {
          "length": 12,
          "result": "success",
          "offset": null
        },
        {
          "length": 13,
          "result": "Truncated",
          "offset": 13
        },
        {
          "length": 14,
          "result": "success",
          "offset": null
        }
      ],
      "sha256": "87629062ca3837e2074830989d1d1c7fa2dcff0c82a147c79caa2649cb3c98a2"
    },
    {
      "name": "duplicates",
      "hex": "12 0a 08 00 08 63 12 00 12 00 18 01 12 04 08 03 18 02 12 00 30 01",
      "N": 22,
      "F": 4,
      "K": 3,
      "M": [
        10,
        4,
        0
      ],
      "G": [
        5,
        2,
        0
      ],
      "j": [
        2,
        0,
        0
      ],
      "header": {
        "class_name": 1
      },
      "arguments": [
        {
          "span": [
            2,
            12
          ],
          "projection": "In",
          "type_id": 1,
          "inline": [
            {
              "span": [
                6,
                6
              ],
              "hex": ""
            },
            {
              "span": [
                8,
                8
              ],
              "hex": ""
            }
          ],
          "raw_hex": "08 00 08 63 12 00 12 00 18 01"
        },
        {
          "span": [
            14,
            18
          ],
          "projection": "Star",
          "type_id": 2,
          "inline": [],
          "raw_hex": "08 03 18 02"
        },
        {
          "span": [
            20,
            20
          ],
          "projection": null,
          "type_id": null,
          "inline": [],
          "raw_hex": ""
        }
      ],
      "success_prefixes": [
        0,
        12,
        18,
        20,
        22
      ],
      "minimum": {
        "records": 15,
        "nodes": 4,
        "depth": 2,
        "owned_A": 4,
        "owned_S": 2,
        "work_constant": 92
      },
      "prefix_outcomes": [
        {
          "length": 0,
          "result": "success",
          "offset": null
        },
        {
          "length": 1,
          "result": "Truncated",
          "offset": 1
        },
        {
          "length": 2,
          "result": "Truncated",
          "offset": 2
        },
        {
          "length": 3,
          "result": "Truncated",
          "offset": 3
        },
        {
          "length": 4,
          "result": "Truncated",
          "offset": 4
        },
        {
          "length": 5,
          "result": "Truncated",
          "offset": 5
        },
        {
          "length": 6,
          "result": "Truncated",
          "offset": 6
        },
        {
          "length": 7,
          "result": "Truncated",
          "offset": 7
        },
        {
          "length": 8,
          "result": "Truncated",
          "offset": 8
        },
        {
          "length": 9,
          "result": "Truncated",
          "offset": 9
        },
        {
          "length": 10,
          "result": "Truncated",
          "offset": 10
        },
        {
          "length": 11,
          "result": "Truncated",
          "offset": 11
        },
        {
          "length": 12,
          "result": "success",
          "offset": null
        },
        {
          "length": 13,
          "result": "Truncated",
          "offset": 13
        },
        {
          "length": 14,
          "result": "Truncated",
          "offset": 14
        },
        {
          "length": 15,
          "result": "Truncated",
          "offset": 15
        },
        {
          "length": 16,
          "result": "Truncated",
          "offset": 16
        },
        {
          "length": 17,
          "result": "Truncated",
          "offset": 17
        },
        {
          "length": 18,
          "result": "success",
          "offset": null
        },
        {
          "length": 19,
          "result": "Truncated",
          "offset": 19
        },
        {
          "length": 20,
          "result": "success",
          "offset": null
        },
        {
          "length": 21,
          "result": "Truncated",
          "offset": 21
        },
        {
          "length": 22,
          "result": "success",
          "offset": null
        }
      ],
      "sha256": "8ce2f93147c6184733f7160e58821c9f97034367ac71c16283afc3fde98d94b3"
    },
    {
      "name": "opaque",
      "hex": "12 02 12 00 2a 01 ff 52 01 00 6a 01 00 10 07",
      "N": 15,
      "F": 5,
      "K": 1,
      "M": [
        2
      ],
      "G": [
        1
      ],
      "j": [
        1
      ],
      "header": {},
      "arguments": [
        {
          "span": [
            2,
            4
          ],
          "projection": null,
          "type_id": null,
          "inline": [
            {
              "span": [
                2,
                2
              ],
              "hex": ""
            }
          ],
          "raw_hex": "12 00"
        }
      ],
      "success_prefixes": [
        0,
        4,
        7,
        10,
        13,
        15
      ],
      "minimum": {
        "records": 11,
        "nodes": 2,
        "depth": 2,
        "owned_A": 1,
        "owned_S": 1,
        "work_constant": 56
      },
      "prefix_outcomes": [
        {
          "length": 0,
          "result": "success",
          "offset": null
        },
        {
          "length": 1,
          "result": "Truncated",
          "offset": 1
        },
        {
          "length": 2,
          "result": "Truncated",
          "offset": 2
        },
        {
          "length": 3,
          "result": "Truncated",
          "offset": 3
        },
        {
          "length": 4,
          "result": "success",
          "offset": null
        },
        {
          "length": 5,
          "result": "Truncated",
          "offset": 5
        },
        {
          "length": 6,
          "result": "Truncated",
          "offset": 6
        },
        {
          "length": 7,
          "result": "success",
          "offset": null
        },
        {
          "length": 8,
          "result": "Truncated",
          "offset": 8
        },
        {
          "length": 9,
          "result": "Truncated",
          "offset": 9
        },
        {
          "length": 10,
          "result": "success",
          "offset": null
        },
        {
          "length": 11,
          "result": "Truncated",
          "offset": 11
        },
        {
          "length": 12,
          "result": "Truncated",
          "offset": 12
        },
        {
          "length": 13,
          "result": "success",
          "offset": null
        },
        {
          "length": 14,
          "result": "Truncated",
          "offset": 14
        },
        {
          "length": 15,
          "result": "success",
          "offset": null
        }
      ],
      "sha256": "c6b4eaea9d0593649eb2b4b528767f9eea924d8443ea5d1478260c20d637f74d"
    },
    {
      "name": "enum",
      "hex": "12 0c 08 01 08 ff ff ff ff 0f 08 63 18 00",
      "N": 14,
      "F": 1,
      "K": 1,
      "M": [
        12
      ],
      "G": [
        4
      ],
      "j": [
        0
      ],
      "header": {},
      "arguments": [
        {
          "span": [
            2,
            14
          ],
          "projection": "Out",
          "type_id": 0,
          "inline": [],
          "raw_hex": "08 01 08 ff ff ff ff 0f 08 63 18 00"
        }
      ],
      "success_prefixes": [
        0,
        14
      ],
      "minimum": {
        "records": 6,
        "nodes": 2,
        "depth": 2,
        "owned_A": 1,
        "owned_S": 0,
        "work_constant": 54
      },
      "prefix_outcomes": [
        {
          "length": 0,
          "result": "success",
          "offset": null
        },
        {
          "length": 1,
          "result": "Truncated",
          "offset": 1
        },
        {
          "length": 2,
          "result": "Truncated",
          "offset": 2
        },
        {
          "length": 3,
          "result": "Truncated",
          "offset": 3
        },
        {
          "length": 4,
          "result": "Truncated",
          "offset": 4
        },
        {
          "length": 5,
          "result": "Truncated",
          "offset": 5
        },
        {
          "length": 6,
          "result": "Truncated",
          "offset": 6
        },
        {
          "length": 7,
          "result": "Truncated",
          "offset": 7
        },
        {
          "length": 8,
          "result": "Truncated",
          "offset": 8
        },
        {
          "length": 9,
          "result": "Truncated",
          "offset": 9
        },
        {
          "length": 10,
          "result": "Truncated",
          "offset": 10
        },
        {
          "length": 11,
          "result": "Truncated",
          "offset": 11
        },
        {
          "length": 12,
          "result": "Truncated",
          "offset": 12
        },
        {
          "length": 13,
          "result": "Truncated",
          "offset": 13
        },
        {
          "length": 14,
          "result": "success",
          "offset": null
        }
      ],
      "sha256": "56127bb178230feddfe87403bdd8a583207577bdd5ec94e92309b4c24a1d813d"
    },
    {
      "name": "signed",
      "hex": "12 0d 18 ff ff ff ff ff ff ff ff ff 01 08 03",
      "N": 15,
      "F": 1,
      "K": 1,
      "M": [
        13
      ],
      "G": [
        2
      ],
      "j": [
        0
      ],
      "header": {},
      "arguments": [
        {
          "span": [
            2,
            15
          ],
          "projection": "Star",
          "type_id": -1,
          "inline": [],
          "raw_hex": "18 ff ff ff ff ff ff ff ff ff 01 08 03"
        }
      ],
      "success_prefixes": [
        0,
        15
      ],
      "minimum": {
        "records": 4,
        "nodes": 2,
        "depth": 2,
        "owned_A": 1,
        "owned_S": 0,
        "work_constant": 53
      },
      "prefix_outcomes": [
        {
          "length": 0,
          "result": "success",
          "offset": null
        },
        {
          "length": 1,
          "result": "Truncated",
          "offset": 1
        },
        {
          "length": 2,
          "result": "Truncated",
          "offset": 2
        },
        {
          "length": 3,
          "result": "Truncated",
          "offset": 3
        },
        {
          "length": 4,
          "result": "Truncated",
          "offset": 4
        },
        {
          "length": 5,
          "result": "Truncated",
          "offset": 5
        },
        {
          "length": 6,
          "result": "Truncated",
          "offset": 6
        },
        {
          "length": 7,
          "result": "Truncated",
          "offset": 7
        },
        {
          "length": 8,
          "result": "Truncated",
          "offset": 8
        },
        {
          "length": 9,
          "result": "Truncated",
          "offset": 9
        },
        {
          "length": 10,
          "result": "Truncated",
          "offset": 10
        },
        {
          "length": 11,
          "result": "Truncated",
          "offset": 11
        },
        {
          "length": 12,
          "result": "Truncated",
          "offset": 12
        },
        {
          "length": 13,
          "result": "Truncated",
          "offset": 13
        },
        {
          "length": 14,
          "result": "Truncated",
          "offset": 14
        },
        {
          "length": 15,
          "result": "success",
          "offset": null
        }
      ],
      "sha256": "c86bf39c23dae12eddc913bb6d3645d17016b3d276ac8d840f47b8433f03eebd"
    },
    {
      "name": "nonminimal",
      "hex": "92 00 84 00 88 00 81 00",
      "N": 8,
      "F": 1,
      "K": 1,
      "M": [
        4
      ],
      "G": [
        1
      ],
      "j": [
        0
      ],
      "header": {},
      "arguments": [
        {
          "span": [
            4,
            8
          ],
          "projection": "Out",
          "type_id": null,
          "inline": [],
          "raw_hex": "88 00 81 00"
        }
      ],
      "success_prefixes": [
        0,
        8
      ],
      "minimum": {
        "records": 3,
        "nodes": 2,
        "depth": 2,
        "owned_A": 1,
        "owned_S": 0,
        "work_constant": 28
      },
      "prefix_outcomes": [
        {
          "length": 0,
          "result": "success",
          "offset": null
        },
        {
          "length": 1,
          "result": "Truncated",
          "offset": 1
        },
        {
          "length": 2,
          "result": "Truncated",
          "offset": 2
        },
        {
          "length": 3,
          "result": "Truncated",
          "offset": 3
        },
        {
          "length": 4,
          "result": "Truncated",
          "offset": 4
        },
        {
          "length": 5,
          "result": "Truncated",
          "offset": 5
        },
        {
          "length": 6,
          "result": "Truncated",
          "offset": 6
        },
        {
          "length": 7,
          "result": "Truncated",
          "offset": 7
        },
        {
          "length": 8,
          "result": "success",
          "offset": null
        }
      ],
      "sha256": "35faa5c45d0cf2313293ff198f22e645c8e4953df055052d83c02f696cbd70cd"
    },
    {
      "name": "wrong_wire",
      "hex": "12 09 0a 01 ff 10 00 1a 00 20 01",
      "N": 11,
      "F": 1,
      "K": 1,
      "M": [
        9
      ],
      "G": [
        4
      ],
      "j": [
        0
      ],
      "header": {},
      "arguments": [
        {
          "span": [
            2,
            11
          ],
          "projection": null,
          "type_id": null,
          "inline": [],
          "raw_hex": "0a 01 ff 10 00 1a 00 20 01"
        }
      ],
      "success_prefixes": [
        0,
        11
      ],
      "minimum": {
        "records": 6,
        "nodes": 2,
        "depth": 2,
        "owned_A": 1,
        "owned_S": 0,
        "work_constant": 45
      },
      "prefix_outcomes": [
        {
          "length": 0,
          "result": "success",
          "offset": null
        },
        {
          "length": 1,
          "result": "Truncated",
          "offset": 1
        },
        {
          "length": 2,
          "result": "Truncated",
          "offset": 2
        },
        {
          "length": 3,
          "result": "Truncated",
          "offset": 3
        },
        {
          "length": 4,
          "result": "Truncated",
          "offset": 4
        },
        {
          "length": 5,
          "result": "Truncated",
          "offset": 5
        },
        {
          "length": 6,
          "result": "Truncated",
          "offset": 6
        },
        {
          "length": 7,
          "result": "Truncated",
          "offset": 7
        },
        {
          "length": 8,
          "result": "Truncated",
          "offset": 8
        },
        {
          "length": 9,
          "result": "Truncated",
          "offset": 9
        },
        {
          "length": 10,
          "result": "Truncated",
          "offset": 10
        },
        {
          "length": 11,
          "result": "success",
          "offset": null
        }
      ],
      "sha256": "d40e4ceca5b71dd3e0bf24bdf1881d832f397ffef4ca50079b588e01614fde54"
    },
    {
      "name": "opaque_inline",
      "hex": "12 03 12 01 00",
      "N": 5,
      "F": 1,
      "K": 1,
      "M": [
        3
      ],
      "G": [
        1
      ],
      "j": [
        1
      ],
      "header": {},
      "arguments": [
        {
          "span": [
            2,
            5
          ],
          "projection": null,
          "type_id": null,
          "inline": [
            {
              "span": [
                2,
                3
              ],
              "hex": "00"
            }
          ],
          "raw_hex": "12 01 00"
        }
      ],
      "success_prefixes": [
        0,
        5
      ],
      "minimum": {
        "records": 3,
        "nodes": 2,
        "depth": 2,
        "owned_A": 1,
        "owned_S": 1,
        "work_constant": 21
      },
      "prefix_outcomes": [
        {
          "length": 0,
          "result": "success",
          "offset": null
        },
        {
          "length": 1,
          "result": "Truncated",
          "offset": 1
        },
        {
          "length": 2,
          "result": "Truncated",
          "offset": 2
        },
        {
          "length": 3,
          "result": "Truncated",
          "offset": 3
        },
        {
          "length": 4,
          "result": "Truncated",
          "offset": 4
        },
        {
          "length": 5,
          "result": "success",
          "offset": null
        }
      ],
      "sha256": "23a0ee9fbbeb0027dc46e543a0e436c8a065eb5d3cdf8c5ae9885c562ed12c19"
    },
    {
      "name": "pair_type",
      "hex": "12 04 12 02 38 00 12 04 12 02 38 01 30 00",
      "N": 14,
      "F": 3,
      "K": 2,
      "M": [
        4,
        4
      ],
      "G": [
        1,
        1
      ],
      "j": [
        1,
        1
      ],
      "header": {
        "class_name": 0
      },
      "arguments": [
        {
          "span": [
            2,
            6
          ],
          "projection": null,
          "type_id": null,
          "inline": [
            {
              "span": [
                2,
                4
              ],
              "hex": "38 00"
            }
          ],
          "raw_hex": "12 02 38 00"
        },
        {
          "span": [
            8,
            12
          ],
          "projection": null,
          "type_id": null,
          "inline": [
            {
              "span": [
                2,
                4
              ],
              "hex": "38 01"
            }
          ],
          "raw_hex": "12 02 38 01"
        }
      ],
      "success_prefixes": [
        0,
        6,
        12,
        14
      ],
      "minimum": {
        "records": 8,
        "nodes": 3,
        "depth": 2,
        "owned_A": 2,
        "owned_S": 2,
        "work_constant": 55
      },
      "prefix_outcomes": [
        {
          "length": 0,
          "result": "success",
          "offset": null
        },
        {
          "length": 1,
          "result": "Truncated",
          "offset": 1
        },
        {
          "length": 2,
          "result": "Truncated",
          "offset": 2
        },
        {
          "length": 3,
          "result": "Truncated",
          "offset": 3
        },
        {
          "length": 4,
          "result": "Truncated",
          "offset": 4
        },
        {
          "length": 5,
          "result": "Truncated",
          "offset": 5
        },
        {
          "length": 6,
          "result": "success",
          "offset": null
        },
        {
          "length": 7,
          "result": "Truncated",
          "offset": 7
        },
        {
          "length": 8,
          "result": "Truncated",
          "offset": 8
        },
        {
          "length": 9,
          "result": "Truncated",
          "offset": 9
        },
        {
          "length": 10,
          "result": "Truncated",
          "offset": 10
        },
        {
          "length": 11,
          "result": "Truncated",
          "offset": 11
        },
        {
          "length": 12,
          "result": "success",
          "offset": null
        },
        {
          "length": 13,
          "result": "Truncated",
          "offset": 13
        },
        {
          "length": 14,
          "result": "success",
          "offset": null
        }
      ],
      "sha256": "c283d87a0cdef8565d5ce2295bf0faae3b53f591cc395da870c6753752f1fe88"
    }
  ],
  "malformed": [
    {
      "name": "parent_zero",
      "hex": "00",
      "error": "InvalidFieldNumber",
      "offset": 0
    },
    {
      "name": "parent_group",
      "hex": "23",
      "error": "UnsupportedGroup",
      "offset": 0
    },
    {
      "name": "parent_wire",
      "hex": "0e",
      "error": "InvalidWireType",
      "offset": 0
    },
    {
      "name": "child_short_after_sibling",
      "hex": "12 00 12 01 18",
      "error": "Truncated",
      "offset": 5
    },
    {
      "name": "child_short_after_nonminimal_parent",
      "hex": "92 00 81 00 18",
      "error": "Truncated",
      "offset": 5
    },
    {
      "name": "child_zero",
      "hex": "12 01 00",
      "error": "InvalidFieldNumber",
      "offset": 2
    },
    {
      "name": "child_short",
      "hex": "12 01 18",
      "error": "Truncated",
      "offset": 3
    },
    {
      "name": "parent_short",
      "hex": "12 02 18",
      "error": "Truncated",
      "offset": 3
    },
    {
      "name": "later_parent_wins",
      "hex": "12 01 00 00",
      "error": "InvalidFieldNumber",
      "offset": 3
    },
    {
      "name": "child_group",
      "hex": "12 01 23",
      "error": "UnsupportedGroup",
      "offset": 2
    },
    {
      "name": "child_wire",
      "hex": "12 01 0e",
      "error": "InvalidWireType",
      "offset": 2
    },
    {
      "name": "child_bulk_short",
      "hex": "12 03 12 02 ff",
      "error": "Truncated",
      "offset": 5
    },
    {
      "name": "child_negative_length",
      "hex": "12 06 12 ff ff ff ff 0f",
      "error": "NegativeLength",
      "offset": 3
    },
    {
      "name": "child_malformed_varint",
      "hex": "12 0a 80 80 80 80 80 80 80 80 80 80",
      "error": "MalformedVarint",
      "offset": 11
    },
    {
      "name": "parent_malformed_varint",
      "hex": "80 80 80 80 80 80 80 80 80 80",
      "error": "MalformedVarint",
      "offset": 9
    }
  ]
}
```

# Exact successful-fixture deficits (CONTRACT; UNEXECUTED)

Use each literal's exact R/Q/D/W/O from EXPECTATIONS.json; all unmentioned limits
are exact successful values. A and S are target Rust size_of, not ABI guesses.
Exact and each independently +1 bound succeed. Empty literal all minima:
bytes0,records0,nodes1,depth1,work1,owned0; explicit Inv/zero/STAR+type and signed
boundaries are covered by unchanged standalone tests and new enum/signed fixtures.

| literal | R-1 records@ | Q-1 nodes@ | depth1 depth@ | W-1 work@ | O-1 owned_bytes@ |
|---|---:|---:|---:|---:|---:|
| selected |12|4|4|12|2|
| duplicates |20|20|2|20|18|
| opaque |13|2|2|13|0|
| enum |12|2|2|0|0|
| signed |13|2|2|0|0|
| nonminimal |4|4|4|0|0|
| wrong_wire |9|2|2|0|0|
| opaque_inline |2|2|2|0|0|
| pair_type |12|8|2|12|6|

All selected child nodes include empty bodies and wrong-wire-only bodies.
bytes=N-1 -> bytes@0 for each full literal, before depth0/nodes0/records0/work0.
A child inline allocation is before its enclosing outer push. O-1 in this table
usually targets outer growth because it happens after the child's own allocations;
for fine child-owned/growth tests use SPEC.md sequences, never alter expected
values to match output. Canonical distinguishing witness is Pair records3 (all independent public parent
and children have <=3 records each) -> records@0 first rescan; exact aggregate8
passes. Pair nodes2 fails second child, owned(2A+2S-1) fails second outer growth,
work(55+2A+2S-1) fails final parent dispatch: aggregate effects cannot be hidden by
per-child caps.

All malformed error fixtures in EXPECTATIONS.json use default limits and errors
relative to supplied Type input. Every proper prefix of a single failing parent
field yields Truncated@end unless tag validity/group/wire already fails; every
parent-complete malformed child fails in child frame shifted once. No field number,
length or tag is inferred from produced decoder data. Future tests should include
all127 valid prefixes and all15 explicit malformed cases, not generate expected
kinds with the implementation under test.
