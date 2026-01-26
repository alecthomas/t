# T - a text processing language and utility

`t` is a concise language for manipulating text, replacing common usage patterns of Unix utilities like grep, sed, cut, awk, sort, and uniq.

> [!WARNING]
> This is a very early draft with only a minimal set of operators.

## Usage

```
t [<flags>] <programme> [<file> ...]
```

## Example - top 20 most frequent words, lowercased

Using traditional Unix utilities:

```bash
tr -s '[:space:]' '\n' < file | tr A-Z a-z | sort | uniq -c | sort -rn | head -20
```

The equivalent in `t` would be:

```bash
t 'sjldo:20' file
```

Going through the programme step by step gives us:

| Op | State | Description |
|----|-------|-------------|
| | `[line, line, ...]` | lines of input |
| `s` | `[[word, word], [word], ...]` | split each line into words |
| `j` | `[word, word, word, ...]` | flatten into single list |
| `l` | `[word, word, word, ...]` | lowercase each word |
| `d` | `[[5, "the"], [3, "cat"], ...]` | dedupe with counts |
| `o` | `[[5, "the"], [3, "cat"], ...]` | sort descending (by count) |
| `:20` | `[[5, "the"], [3, "cat"], ...]` | take first 20 |

## Data Model

By default, input is a flat stream of lines, with each input file's lines concatenated together: `[line, line, ...]`. The `-f` flag can be used to switch to file mode: `[file, file, ...]`.

Operators come in three kinds:

- **Transform** (map): apply to each element. `l` on `["Hello", "World"]` → `["hello", "world"]`
- **Filter**: keep or remove elements. `/x/` on `["ax", "by", "cx"]` → `["ax", "cx"]`
- **Reduce**: collapse array to a value. `#` on `[a, b, c]` → `3`

Element-wise transforms (`u`, `l`, `t`, `n`, `r`, `+`) recurse through nested arrays automatically. Structural operators (`d`, `#`, `o`, `O`, selection) operate on the top-level array only—use `@` to apply them at deeper levels.

Selection (`0`, `:3`, `0,2,4`) is a reduce operator—it collapses the array to a subset. To apply selection within each element of a nested structure, use `@` to descend first.

Use `@` to descend into nested structures, `^` to ascend back up. 

## Type System

There are three types:

| Type | Description |
|------|-------------|
| array | ordered collection of values |
| string | text |
| number | numeric value (converted from string via `n`) |

Input is always an array of strings (lines). Operators like `s` create nested arrays, `j` flattens them. Numbers only exist after explicit conversion with `n`, and are used by numeric operators like `+`.

## Split/Join Semantics

Strings have a semantic "level" that affects how `s` splits and `j` joins:

| Level | `s` splits into | `j` joins with |
|-------|-----------------|----------------|
| file | lines | newline |
| line | words | space |
| word | chars | nothing |

`j` also flattens arrays: `[[a, b], [c]]` → `[a, b, c]`

## Operators

### Quick Reference

#### Split/Join

| Operator | Meaning |
|----------|---------|
| `s` | split natural |
| `S<char>` or `S"<delim>"` | split on delimiter |
| `j` | join/flatten natural |
| `J<char>` or `J"<delim>"` | join with delimiter |

#### Transform

| Operator | Meaning |
|----------|---------|
| `l` | lowercase |
| `L<selection>` | lowercase selected |
| `u` | uppercase |
| `U<selection>` | uppercase selected |
| `r[<selection>]/<old>/<new>/` | replace (regex), optionally in selected |
| `n` | to number |
| `N<selection>` | to number selected |
| `t` | trim whitespace |
| `T<selection>` | trim selected |

#### Filter

| Operator | Meaning |
|----------|---------|
| `/<regex>/` | keep matching |
| `!/<regex>/` | keep non-matching |
| `x` | delete empty |

#### Reduce

| Operator | Meaning |
|----------|---------|
| `<selection>` | select elements (index, slice, or multi) |
| `o` | sort descending |
| `O` | sort ascending |
| `g<selection>` | group by |
| `d` | dedupe with counts |
| `D` | dedupe |
| `#` | count |
| `+` | sum |
| `c` | columnate |

#### Navigation

| Operator | Meaning |
|----------|---------|
| `@` | descend |
| `^` | ascend |

### Operator Details

#### `s` - Split

Splits each element according to its semantic level:

- **file** → splits into lines (on newlines)
- **line** → splits into words (on whitespace)
- **word** → splits into characters

```
["hello world", "foo bar"]  →  [["hello", "world"], ["foo", "bar"]]
```

#### `S<delim>` - Split on Delimiter

Splits on a custom delimiter. Use a single character directly, or quotes for multi-character delimiters:

- `S,` splits on comma
- `S:` splits on colon
- `S"::"` splits on `::`

```
# Split CSV
"a,b,c"  →  ["a", "b", "c"]   (with S,)

# Split on ::
"a::b::c"  →  ["a", "b", "c"]   (with S"::")
```

#### `j` - Join/Flatten

Behavior depends on the array contents:

- **Array of arrays**: flattens one level into a single array
- **Array of strings/numbers**: joins with space into a single string

```
# Flatten arrays
[["a", "b"], ["c", "d"]]  →  ["a", "b", "c", "d"]

# Join strings
["hello", "world"]  →  "hello world"
```

#### `J<delim>` - Join with Delimiter

Joins array elements with a custom delimiter:

- `J,` joins with comma
- `J"\n"` joins with newline

```
["a", "b", "c"]  →  "a,b,c"   (with J,)
```

#### `l` - Lowercase

Converts all text to lowercase. Works recursively on arrays.

```
["Hello", "WORLD"]  →  ["hello", "world"]
```

#### `L<selection>` - Lowercase Selected

Lowercases only the elements at the specified indices:

```
["HELLO", "WORLD", "FOO"]  →  ["hello", "WORLD", "FOO"]   (with L0)
["HELLO", "WORLD", "FOO"]  →  ["hello", "world", "FOO"]   (with L:2)
```

#### `u` - Uppercase

Converts all text to uppercase. Works recursively on arrays.

```
["Hello", "world"]  →  ["HELLO", "WORLD"]
```

#### `U<selection>` - Uppercase Selected

Uppercases only the elements at the specified indices.

#### `r[<selection>]/<old>/<new>/` - Replace (Regex)

Replaces matches of regex `<old>` with `<new>`. Recurses through nested arrays.

With an optional selection, applies replacement only to elements at the specified indices.

```
# Remove prefix
["ERROR: fail", "ERROR: crash"]  →  ["fail", "crash"]   (with r/ERROR: //)

# Replace pattern
["cat", "hat"]  →  ["dog", "hat"]   (with r/cat/dog/)

# Replace only in first element
["cat", "cat"]  →  ["dog", "cat"]   (with r0/cat/dog/)
```

#### `n` - To Number

Converts strings to numbers. Recurses through nested arrays. Non-numeric strings error.

```
["42", "3.14", "100"]  →  [42, 3.14, 100]
```

#### `N<selection>` - To Number Selected

Converts only the elements at the specified indices to numbers.

#### `t` - Trim

Removes leading and trailing whitespace from each string. Recurses through nested arrays.

```
["  hello  ", "\tworld\n"]  →  ["hello", "world"]
```

#### `T<selection>` - Trim Selected

Trims only the elements at the specified indices.

#### `/<regex>/` - Filter Keep

Keeps only elements matching the regex.

```
["apple", "banana", "apricot"]  →  ["apple", "apricot"]   (with /^a/)
```

#### `!/<regex>/` - Filter Remove

Removes elements matching the regex (keeps non-matching).

```
["apple", "banana", "apricot"]  →  ["banana"]   (with !/^a/)
```

#### `x` - Delete Empty

Removes empty strings and empty arrays from the current array.

```
["hello", "", "world", ""]  →  ["hello", "world"]
```

#### `<selection>` - Select

Selects elements by index, slice, or combination. See [Selection](#selection) for full syntax.

- Single index returns the element itself
- Multiple indices or slices return an array

Also works on strings, treating them as character arrays:

```
"hello"  →  "h"       (with 0)
"hello"  →  "olleh"   (with ::-1)
```

#### `o` - Sort Descending

Sorts the array in descending order. For arrays of arrays, sorts lexicographically (first element, then second, etc.).

```
[3, 1, 4, 1, 5]  →  [5, 4, 3, 1, 1]
[[2, "b"], [1, "a"], [2, "a"]]  →  [[2, "b"], [2, "a"], [1, "a"]]
```

#### `O` - Sort Ascending

Sorts the array in ascending order.

```
[3, 1, 4, 1, 5]  →  [1, 1, 3, 4, 5]
```

#### `g<selection>` - Group By

Groups elements by the value(s) at the specified selection. Produces `[[key, [elements...]], ...]`.

```
# Group by first element
[["a", 1], ["b", 2], ["a", 3]]  →  [["a", [["a", 1], ["a", 3]]], ["b", [["b", 2]]]]   (with g0)

# Group by slice (composite key)
g0:2  →  key is [first, second] elements
```

#### `d` - Dedupe with Counts

Removes duplicates and counts occurrences. Returns `[[count, value], ...]` sorted by count descending.

```
["a", "b", "a", "a", "b"]  →  [[3, "a"], [2, "b"]]
```

#### `D` - Dedupe

Removes duplicates, keeping first occurrence. Returns unique values only.

```
["a", "b", "a", "a", "b"]  →  ["a", "b"]
```

#### `#` - Count

Returns the number of elements in the array.

```
["a", "b", "c"]  →  3
```

#### `+` - Sum

Sums all numeric values. Recurses through nested arrays. Strings are coerced to numbers (non-numeric strings contribute 0).

```
[1, 2, 3, 4]  →  10
[["1", "2"], ["3", "4"]]  →  10
```

#### `c` - Columnate

Formats array of arrays as aligned columns (like `column -t`).

```
[["name", "age"], ["alice", "30"], ["bob", "25"]]
→
name   age
alice  30
bob    25
```

#### `@` - Descend

Descends one level into the data structure. Subsequent operations apply to each element of the current array, rather than the array itself.

```
# Without @: select first element of outer array
[["a", "b"], ["c", "d"]]  →  ["a", "b"]   (with 0)

# With @: select first element of EACH inner array
[["a", "b"], ["c", "d"]]  →  ["a", "c"]   (with @0)
```

Multiple `@` descends multiple levels:

```
# @@0 operates on elements of elements of elements
```

#### `^` - Ascend

Ascends one level, undoing a previous `@`. Returns focus to the parent array.

```
# Split, descend, select first word, ascend, join
"hello world\nfoo bar"  →  ["hello", "foo"]  →  "hello foo"   (with s@0^j)
```

## Selection

Selection is a reduce operator—it collapses the array to a subset. Selecting a single element returns that element; selecting multiple returns an array:

| Syntax | Meaning | Result |
|--------|---------|--------|
| `<n>` | single index (0-based) | element |
| `-<n>` | negative index (from end) | element |
| `<n>:<m>` | slice (exclusive end) | array |
| `<n>:` | slice to end | array |
| `:<m>` | slice from start | array |
| `<n>:<m>:<s>` | slice with stride | array |
| `<n>,<m>,<p>` | select multiple | array |
| `<n>,<m>:<p>` | mixed index + slice | array |

To apply selection within each element of a nested structure, use `@` to descend first:

```bash
# Select first 3 lines
t ':3' file

# Split lines into words, then select first word of each line
t 's@0' file

# Split on colon, select first and last fields of each line
t 'S:@0,-1' /etc/passwd

# Split into words, select 1st, 3rd, 4th of each line
t 's@0,2,3' file

# Reorder columns: last column first, then rest
t 's@-1,0:-1' file
```

## Grouping

`g<selection>` groups elements by the value(s) at the specified selection, producing `[[key, [element, ...]], ...]`.

| Syntax | Meaning |
|--------|---------|
| `g0` | group by first element |
| `g-1` | group by last element |
| `g1,2` | group by composite key (elements 1 and 2) |
| `g0:3` | group by first three elements as key |

Examples:

```bash
# Group log lines by IP (first field)
t 'sg0' access.log
# → [["192.168.1.1", [[192.168.1.1, -, -, ...], ...]], ["10.0.0.5", [...]], ...]

# Group CSV rows by region (field 2)
t 'S,g2' sales.csv

# Group by composite key: method + status code
t 'sg0,8' access.log

# Group by IP (first field), showing all requests per IP
t 'sg0' access.log
# → [["192.168.1.1", [[...], [...]]], ["10.0.0.5", [[...]]]]

# Group by IP, show top 10 offenders with their actual requests
t 'sg0o:10' access.log
```

## Aggregation & Cleaning

| Operator | Behavior | Example |
|----------|----------|---------|
| `#` | count: `[a, b, c]` → `3` | `t '#' file` (line count) |
| `+` | sum: `[1, 2, 3]` → `6` | `t 'S,@1n+' data.csv` (sum column 2) |
| `t` | trim whitespace (per element) | `t 't' file` (trim each line) |
| `x` | delete empty elements | `t 'x' file` (remove blank lines) |

## Interactive Mode

```bash
$ t -i access.log
Loaded 124847 lines
t> s                     # live preview as you type
[[192.168.1.1, -, -, ...], [10.0.0.5, -, -, ...], ...]
t> s@
[[192.168.1.1, -, -, ...], [10.0.0.5, -, -, ...], ...]
t> s@8
["200", "404", "200", "500", ...]
t> s@8d
[[98423, "200"], [1042, "404"], [89, "500"], ...]
t> s@8do
[[98423, "200"], [1042, "404"], [89, "500"], ...]
t> s@8do:10<Enter>       # enter commits
```

## CLI Flags

| Flag | Meaning |
|------|---------|
| `-d <delim>` | input delimiter (what `s` splits on) |
| `-D <delim>` | output delimiter (what `j` joins with) |
| `-f` | file mode |
| `-e <prog>` | explain |
| `-p <prog>` | parse tree |
| `-i` | interactive |
| `-j` | json output |

## Rosetta Stone

### Filtering

**Lines with "fail" but not "expected":**
```bash
grep fail file | grep -v expected
t '/fail/!/expected/' file
```

**Error messages, deduped and sorted by frequency:**
```bash
grep ERROR app.log | sed 's/.*ERROR: //' | sort | uniq -c | sort -rn
t '/ERROR/r/.*ERROR: //do' app.log
```

### Field Selection

**Select specific columns (1st, 3rd, 4th) from whitespace-delimited file:**
```bash
awk '{print $1, $3, $4}' file
t 's@0,2,3' file
```

**Extract username and shell from /etc/passwd:**
```bash
awk -F: '{print $1, $7}' /etc/passwd
t 'S:@0,-1' /etc/passwd
```

**Reorder CSV columns (swap first two, keep rest):**
```bash
awk -F, -v OFS=, '{print $2, $1, $3}' file
t -d, -D, 's@1,0,2:j' file
```

**Colon-delimited: 5th field, lowercased, reversed:**
```bash
cut -d: -f5 /etc/passwd | tr A-Z a-z | rev
t 'S:@4ls::-1j' /etc/passwd
```

### Grouping

**Group log lines by IP, see all requests from each:**
```bash
# No simple Unix equivalent - requires awk with arrays
awk '{a[$1] = a[$1] ? a[$1] "\n" $0 : $0} END {for (k in a) print "==" k "==\n" a[k]}' access.log
t 'sg0' access.log
```

**Group errors by error type, show all occurrences:**
```bash
# Complex in traditional tools
t '/ERROR/r/.*ERROR: //sg0' app.log
```

**Group by field:**
```bash
# No simple Unix equivalent - requires awk with arrays
t 'sg0' access.log
```

**Group CSV by category, sum values per category:**
```bash
awk -F, '{sum[$3]+=$2} END {for (k in sum) print k, sum[k]}' data.csv
t 'S,g2@1@1n+' data.csv
```

### Frequency & Deduplication

**Request counts by IP (first field of log):**
```bash
awk '{print $1}' access.log | sort | uniq -c | sort -rn
t 's@0do' access.log
```

**HTTP status code distribution (9th field):**
```bash
awk '{print $9}' access.log | sort | uniq -c | sort -rn
t 's@8do' access.log
```

**Most requested URLs (7th field), top 20:**
```bash
awk '{print $7}' access.log | sort | uniq -c | sort -rn | head -20
t 's@6do:20' access.log
```

**Top 10 file extensions:**
```bash
ls -1 | grep '\.' | rev | cut -d. -f1 | rev | sort | uniq -c | sort -rn | head -10
t '/\./S.@-1do:10' filelist
```

**CSV: value frequency in column 1:**
```bash
cut -d, -f1 data.csv | sort | uniq -c | sort -rn
t 'S,@0do' data.csv
```

**CSV: unique values in column 3, sorted:**
```bash
cut -d, -f3 data.csv | sort -u
t 'S,@2DO' data.csv
```

**Extract and count email domains:**
```bash
grep -E '@' file | sed 's/.*@//' | sed 's/[^a-zA-Z0-9.-].*//' | sort | uniq -c | sort -rn
t '/@/S@@-1do' file
```

**Remove duplicate words within each line:**
```bash
awk '{delete a; for(i=1;i<=NF;i++) if(!a[$i]++) printf "%s ", $i; print ""}' file
t 's@Dj' file
```

### Counting & Aggregation

**Count lines (like wc -l):**
```bash
wc -l < file
t '#' file
```

**Count words (like wc -w):**
```bash
wc -w < file
t 'sj#' file
```

**Sum a column of numbers:**
```bash
awk '{sum+=$1} END{print sum}' file
t 'n+' file
```

**Sum column 2 of a CSV:**
```bash
awk -F, '{sum+=$2} END{print sum}' data.csv
t 'S,@1n+' data.csv
```

### Cleaning & Transformation

**Remove blank lines:**
```bash
sed '/^$/d' file
t 'x' file
```

**Trim whitespace from each line:**
```bash
sed 's/^[ \t]*//;s/[ \t]*$//' file
t 't' file
```

**Reverse words within each line:**
```bash
awk '{for(i=NF;i>=1;i--) printf "%s ", $i; print ""}' file
t 's@::-1j' file
```

**Reverse each word's characters (hello world → olleh dlrow):**
```bash
# Bash equivalent is ugly:
while IFS= read -r line; do echo "$line" | xargs -n1 | rev | xargs; done < file
t 's@s@::-1^j^j' file
```

### Slicing

**First 5 lines of each file:**
```bash
head -5 a.txt b.txt c.txt
t -f 's:5' a.txt b.txt c.txt
```

**Every 3rd line, starting from line 2:**
```bash
awk 'NR%3==2' file
t '1::3' file
```

### Multi-file Operations

**Word frequency per file (top 10 each):**
```bash
for f in *.txt; do echo "==$f=="; tr -s '[:space:]' '\n' < "$f" | sort | uniq -c | sort -rn | head -10; done
t -f 's@sjdo:10' *.txt
```
