---
title: Embedded values and variables
description: Use Ironbullet's <...> placeholders, input slices, named HTTP response trees, captures, and globals.
---

# Embedded values and variables

Ironbullet passes values between blocks through a scoped variable store. The syntax is deliberately explicit: **use angle brackets in template text**.

```text
https://example.test/session/<USER>?token=<TOKEN>
```

The placeholder is resolved when the block executes. It is not Markdown syntax and it is not OpenBullet bracket syntax.

## The rule to remember

| Where you are typing | Correct form | Example |
| --- | --- | --- |
| URL, header value, request body, log message, selector, or other template text | Wrap the name in `<` and `>` | `Bearer <TOKEN>` |
| A field labelled **Input variable** | A bare variable name works; a template also works | `data.LOGIN` or `<data.LOGIN>` |
| A literal that happens to contain brackets | Leave it literal | `[USER]` stays `[USER]` |

Unresolved placeholders are left unchanged. If Debug output still contains `<MISSING>`, the name, scope, capitalization, or preceding block is wrong.

## Wordlist input

A new credential pipeline uses `:` as its separator with the slices `USER` and `PASS`. For a row such as:

```text
alice:correct-horse
```

use either the explicit scoped form or the shorthand:

```text
<input.USER>    → alice
<USER>          → alice
<input.PASS>    → correct-horse
<PASS>          → correct-horse
```

The shorthand works because bare names fall back to input slices after user and response values. The scoped form is clearer in shared configurations and is recommended when a name could collide with a capture.

Change the **Data** panel's separator and slice names for another input layout. For example, a list configured with `EMAIL,PASS` exposes `<input.EMAIL>` and `<input.PASS>`.

## HTTP response variables

Every HTTP Request has a **Response variable**. The default is `SOURCE`; use a descriptive value such as `LOGIN` when a pipeline has more than one request.

If the block is named `LOGIN`, its result tree is:

| Value | Placeholder |
| --- | --- |
| Response body | `<data.LOGIN>` |
| Status code | `<data.LOGIN.STATUS>` |
| Final URL after redirects | `<data.LOGIN.URL>` |
| All response headers as JSON | `<data.LOGIN.HEADERS>` |
| One header | `<data.LOGIN.HEADERS.content-type>` |
| All response cookies as JSON | `<data.LOGIN.COOKIES>` |
| One cookie | `<data.LOGIN.COOKIES.session>` |
| Request error | `<data.LOGIN.ERROR>` |

Header names are normalized to lowercase in individual header entries. Cookie names retain the name supplied by the response. On a failed request, Ironbullet clears the prior response tree first, so an old cookie or status cannot be reused accidentally.

For the default response variable, replace `LOGIN` with `SOURCE`: `<data.SOURCE>`, `<data.SOURCE.STATUS>`, and `<data.SOURCE.COOKIES.session>`.

## Parser outputs and captures

A parser or function block writes its configured output variable into the user-variable scope. If a Parse JSON block writes `TOKEN`, use:

```text
Authorization: Bearer <TOKEN>
```

`<data.TOKEN>` also resolves for compatibility, but `<TOKEN>` is the preferred form for a named capture. Mark the block output as a capture when it should be retained with job hits.

## Globals and custom inputs

Custom inputs supplied when a job starts are placed in the globals scope:

```text
<globals.API_BASE>
<globals.REGION>
```

Use globals for run-level configuration that should not come from a wordlist line or a response.

## Random inline values

These placeholders generate a value each time they are resolved:

```text
<random.uuid>
<random.email>
<random.phone>
<random.string>
<random.string.32>
<random.number>
<random.number.1.100>
<random.name.full>
```

Do not reference the same random placeholder twice when the two fields must match. Generate it once into a named variable, then use that variable in both places.

## Migrating common OpenBullet-style references

| Existing form | Ironbullet form | Notes |
| --- | --- | --- |
| `[USER]` | `<USER>` or `<input.USER>` | Square brackets are literal in Ironbullet. |
| `[PASS]` | `<PASS>` or `<input.PASS>` | Uses the configured wordlist slice. |
| `data.cookies` | `<data.SOURCE.COOKIES>` | Use the named request tree; the default response variable is `SOURCE`. |
| `data.cookies["session"]` | `<data.SOURCE.COOKIES.session>` | Use the actual cookie name after the dot. |
| `response.data` | `<data.SOURCE>` | Or `<data.LOGIN>` when the request response variable is `LOGIN`. |
| `response.status` | `<data.SOURCE.STATUS>` | Prefer the named response tree over legacy aliases. |
| `@token` | `<TOKEN>` | User variables do not use an `@` prefix. |

## Debug checklist

1. Give each HTTP Request a unique response variable, such as `LOGIN` or `PROFILE`.
2. Run one authorized input in **Debug**.
3. Inspect the variable snapshot after the request.
4. Copy the exact key into the next block, wrapping it in `<...>` for template fields.
5. If a placeholder remains literal, confirm the preceding block ran and the exact case matches.

See [Pipeline model](/Ironbullet/pipeline-model/) for scopes and [HTTP requests](/Ironbullet/http-requests/) for response-state behavior.
