# _md

[![Tests](https://github.com/skial/serve_md/actions/workflows/test.yaml/badge.svg)](https://github.com/skial/serve_md/actions/workflows/test.yaml)

Just another commonmark/markdown render.

#### Options

- Footnotes
- Smart Punctuation
- Header attributes
- GitHub flavoured tables, task lists & strikethrough.
- Front matter parsing _(either YAML, JSON, TOML or Refdef)_.
    - A Refdef is any _simple_ [link reference definition](https://spec.commonmark.org/0.30/#link-reference-definitions) that precedes the main content.
    - Simple in the sense that the link reference definition fits on a single line.
- Collaspible headers
    - Turns specific headers into:
        ```html
        <details>
                <summary>header text</summary>
                content
        </details>
        ```

#### Why?

This project started out _(and continues)_ as a way to get more familiar with the Rust language, its various libraries, the tooling and the wider ecosystem.

## parse_md

Processes specified input `.md` file to stdout or specified output file.

<details>

<summary>Cli overview</summary>

```text
Usage: parse_md [OPTIONS]

Options:
  -i, --file <FILE>

  -o, --output <OUTPUT>

  -t, --tables
          Enables parsing tables
  -f, --footnotes
          Enables parsing footnotes
  -s, --strikethrough
          Enables parsing strikethrough
  -l, --tasklists
          Enables parsing tasklists
  -p, --smart-punctuation
          Enables smart punctuation
  -a, --header-attributes
          Enables header attributes
  -m, --front-matter <FRONT_MATTER>
          The type of front matter [possible values: refdef, json, yaml, toml]
  -e, --emoji-shortcodes
          Enables parsing emoji shortcodes, using GitHub flavoured shortcodes
  -k, --collapsible-headers <COLLAPSIBLE_HEADERS>
          Enables converting headers into collapsible sections using the <details> element
  -c, --config <CONFIG>
          Use a configuration file instead
  -h, --help
          Print help
```

</details>

## serve_md

Starts a server and maps incoming requests to `.md` files.

<details>

<summary>Cli overview</summary>

```text
Usage: serve_md [OPTIONS]

Options:
      --root <ROOT>
          The root directory to serve .md files from
      --port <PORT>
          The port to bind the serve_md server too [default: 8083]
  -t, --tables
          Enables parsing tables
  -f, --footnotes
          Enables parsing footnotes
  -s, --strikethrough
          Enables parsing strikethrough
  -l, --tasklists
          Enables parsing tasklists
  -p, --smart-punctuation
          Enables smart punctuation
  -a, --header-attributes
          Enables header attributes
  -m, --front-matter <FRONT_MATTER>
          The type of front matter [possible values: refdef, json, yaml, toml]
  -e, --emoji-shortcodes
          Enables parsing emoji shortcodes, using GitHub flavoured shortcodes
  -k, --collapsible-headers <COLLAPSIBLE_HEADERS>
          Enables converting headers into collapsible sections using the <details> element
  -c, --config <CONFIG>
          Use a configuration file instead
  -h, --help
          Print help

```

</details>