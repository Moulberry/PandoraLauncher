# oval

(oval is a fork of [circular](https://github.com/sozu-proxy/circular) with a
couple more PRs merged in).

oval is a stream abstraction designed for use with nom. It can expose the
available data, a mutable slice of the available space, and it separates reading
data from actually consuming it from the buffer.
