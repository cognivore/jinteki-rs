# The textual card DSL — withdrawn

**Read [EDSL.md](EDSL.md) instead.** This file described an external text
format for writing cards: `.cards` files with one indented line per printed
sentence, read by a parser and denoted into the rules engine's vocabulary. It
worked, and both priority decks were written in it. It is no longer the way
cards are written here.

The judgment, recorded honestly because it is the interesting part: a text
format that covers Netrunner's *real* weirdness — Punitive-Counterstrike-class
predicates over game history, costs of X the payer announces, hosting a card in
the middle of an access — is not a card project, it is a language project.
Every card that did not fit demanded new syntax, and the syntax could only ever
lag the engine. The parser also had a failure mode the embedded version cannot
have: the guide it was the contract for promised verbs (`gain 1 click`,
`run hq`, `access N additional cards`) that had never been implemented, and
nothing caught it for a whole wave.

Cards are now written in an **embedded** DSL — typed Rust builders over the
same engine vocabulary. The pedagogy is unchanged and deliberately so: copy the
printed text first, one printed sentence per call, never approximate — write
the marker. What changed is who checks it. A sentence the vocabulary cannot say
now fails to compile rather than failing to parse, and every example in the new
guide is a doctest, so the guide cannot over-promise again.

The parser, the denoter and the `.cards` files are in the history at `1dc87d6`
for anyone who wants to read them.
