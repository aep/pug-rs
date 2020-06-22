DED
====================

since i no longer work with rust, this crate is canceled as well.
it is fairly simple anyway and if i ever work with pug/jade again, i'll probably reimplement it in zetz.


pug (jade) templates
---------------------

reimplemented in rust for performance reasons.

| pug.js | pug-rs |
|--------|--------|
| 780ms  | 29ms   |


usage:
-------

```
$ cargo install pug
$ pug < thing.pug > thing.html
```


with webpack:
------------


pug_loader.js:
```javascript
const spawnSync = require('child_process').spawnSync;
module.exports = function(source) {
  var proc = spawnSync("pug", {
    input: source
  });
  if (proc.status != 0) {
    throw proc.error;
  }
  return proc.stdout.toString();
}
```

```
  module: {
    rules: [
      {
        test: /\.pug$/,
        use: [require.resolve('./pug_loader.js')]
      },

```
