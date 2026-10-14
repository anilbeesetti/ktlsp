# Frozen literal inputs and expected outcomes

Contract v1 contains the exact CP context, stage limits, and meaning of error abbreviations. These inputs were authored before any product/compiler/native execution.

| name | payload hex | expected |
|---|---|---|
| empty | 00 00 | present empty attribute |
| parameter0 | 00 01 00 00 00 00 03 00 00 | TypeParameter0 |
| parameter255 | 00 01 00 ff 00 00 03 00 00 | TypeParameter255 |
| supertype0 | 00 01 10 00 00 00 00 03 00 00 | Supertype0 |
| supertype256 | 00 01 10 01 00 00 00 03 00 00 | Supertype256 |
| superclass | 00 01 10 ff ff 00 00 03 00 00 | Supertype65535 sentinel unchanged |
| bound0 | 00 01 11 00 00 00 00 03 00 00 | Bound parameter0,bound0 |
| bound255 | 00 01 11 ff ff 00 00 03 00 00 | Bound parameter255,bound255 |
| mixed | 00 03 00 ff 00 00 03 00 00 10 ff ff 00 00 03 00 00 11 00 ff 00 00 03 00 00 | parameter255,super65535,bound0/255 |
| all_steps | 00 01 10 00 01 04 00 00 01 00 02 00 03 ff 00 03 00 00 | Supertype1,path[(0,0),(1,0),(2,0),(3,255)] |
| int_bits | 00 01 11 02 03 00 00 03 00 01 00 04 49 00 05 | Bound2/3,x Int0x80000001 |
| utf16 | 00 01 00 01 00 00 03 00 01 00 06 73 00 07 | Parameter1,s String[0,0xd800] |
| missing_parameter | 00 01 00 | Truncated |
| missing_supertype | 00 01 10 ff | Truncated |
| missing_bound | 00 01 11 00 | Truncated |
| missing_path_length | 00 01 11 00 01 | Truncated |
| invalid_kind | 00 01 00 00 01 04 00 00 03 00 00 | Path |
| invalid_argument | 00 01 10 ff ff 01 02 01 00 03 00 00 | Path |
| invalid_kind_missing_index | 00 01 11 00 00 01 ff | Truncated |
| illegal_field | 00 01 13 | Target |
| illegal_method | 00 01 01 | Target |
| illegal_code | 00 01 40 | Target |
| legal_then_illegal | 00 02 00 00 00 00 03 00 00 14 | Target |
| illegal_then_legal | 00 02 14 00 00 00 03 00 00 | Target |
| zero_descriptor | 00 01 00 00 00 00 00 00 00 | Zero |
| wrong_cp | 00 01 10 00 00 00 00 03 00 01 00 04 49 00 07 | Type |
| count_eof | 00 02 11 00 00 00 00 03 00 00 | Truncated |
| trailing | 00 00 00 | Trailing |
