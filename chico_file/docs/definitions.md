### Definitions

#### Sample 1
Route Request to directory
```
[SiteName] {
    serve [Directory]
}
```
The `serve` keyword says that every request arrive from `SiteName` should be served from `directory`.

#### Example

```
www.google.com {
    serve \etc\sites\google\
}
```

#### Sample 2
Respond Status code
```
[SiteName] {
    respond [Directory]
}
```
The `serve` keyword says that every request arrive from `SiteName` should be served from `Directory`.

#### Example

```
www.google.com {
    respond 404
}
```

The `respond` keyword says that every request arrive from `SiteName` should be respond by status code `StatusCode`.