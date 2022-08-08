# gRPC-Web End-to-end Hello World

Out of morbid curiousity, I decided to make an end-to-end gRPC-Web setup with a
backend server in Rust, deployable as a standalone Docker image.

I wanted to see how gRPC with server-side streaming would compare to WebSockets,
so I decided to build a Hello World-ish app where when you open the page in your
browser, it subscribes you to a stream of updates that tells you how many total
subscriptions there are and prints that number on the page. So if you were to
open the page in multiple tabs, the number should change in all tabs to be the
number of currently open tabs, and it should go back down as the tabs are
closed.

I'm kind of bad at computers, so this took me a lot longer than I was hoping.
Here's some things that went wrong:

## Frontend code

- At this moment, the latest version of the Protobuf compiler doesn't work for
  generating JS code! It's been like this for a full month, which is kind of
  mind-blowing. See the [GitHub
  issue](https://github.com/protocolbuffers/protobuf-javascript/issues/127).

  - To solve this, I thought I'd put in some legwork and build the protobuf-js
    plugin from source as described in @johejo's
    [instructions](https://github.com/protocolbuffers/protobuf-javascript/issues/127#issuecomment-1204202870).
    The build failed when I ran it on my Mac (and I went through all the trouble
    of installing Bazel too). Next I tried doing the build in a Docker
    container, which succeeded, but then the executable wouldn't work on my Mac
    outside the container. Rather than putting more time into this, I just
    downgraded protoc as described by @clehene
    [here](https://github.com/protocolbuffers/protobuf-javascript/issues/127#issuecomment-1204202844).

- Generated JS protobuf files greatly increase the frontend bundle size, adding
  230 kB parsed and 46 kB gzipped. This is because the generated files depend on
  `google-protobuf`, which is a gigantic dependency seemingly not subject to
  treeshaking or dead-code elimination. This is very shitty, and at the moment
  it seems that anyone using gRPC-Web just has to live with it.

- When you want to use server-side streaming, you need to put your generated
  gRPC stuff into "text mode", meaning it sends all its protobuf messages as
  base-64 strings rather than binary. Hopefully the size increase is negated by
  compression, but it still feels a little wasteful.

## Vanilla gRPC with Rust

My next goal was to run a normal Rust gRPC server based on
[Tonic](https://github.com/hyperium/tonic) (not gRPC web) out of a container.

- At first, this worked fine locally but failed in the container. As it turned
  out, the issue was this line, copied from an example:

  ```rust
  let addr = "[::1]:50051".parse()?;
  ```

  For reasons I don't understand, to get this to work out of a Docker container
  and not just locally, I needed to change it to

  ```rust
  let addr = "[::]:50051".parse()?;
  ```

- The only other tricky part of the Rust implementation was detecting when the
  client has cancelled out of a server-side stream. The way Tonic indicates this
  is that the `Stream` that our method returns is dropped, so we need to provide
  a custom `Stream` implementation that has a custom implementation of the
  `Drop` trait. This conceptually makes sense (it means we'll never end up
  hanging on to a `Stream` instance after the user has left), but felt a bit
  clunky to actually work with. I could probably make this better by writing
  some helpers.

- I'm glad I did this custom `Stream` implementation because it requires this
  method to be implemented:

  ```rust
  fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>>
  ```

  which forced me to look into the `Pin` trait a little bit, which in the past
  has always scared me off every time I tried to read [the
  docs](https://doc.rust-lang.org/std/pin/index.html). For a while I thought I
  would have to implement what the docs call [structural
  pinning](https://doc.rust-lang.org/std/pin/index.html#pinning-is-structural-for-field)
  which is a little worrying because it uses `unsafe` code, but as it turns out
  I didn't need to get into that at all.

- I spent some time trying to get the build to work on Alpine just for the
  challenge. I couldn't get it to work and I think it's not worth the trouble. I
  can afford my images being an extra 20 MB, plus it'll get much worse later
  anyways because of Envoy (see below).

# To gRPC-Web

Now to turn gRPC into gRPC-Web. This means we need to run an Envoy reverse-proxy
server to do something or other involving turning the browser's HTTP/1.1 into
the HTTP/2 our gRPC server understands.

- Running this locally at first wasn't bad. I spun up an Envoy container and
  gave it the example configuration from the gRPC-Web docs. The only thing I
  needed change from the example was

  ```yaml
  socket_address:
    address: 0.0.0.0
    port_value: 9090
  ```

  to

  ```yaml
  socket_address:
    address: host.docker.internal
    port_value: 50051
  ```

  since we want the container to forward to the locally running Rust server on
  my host machine (and at a different port than the example).

- One extremely frustrating bug here: while the example named the config file
  `envoy.yaml`, I was naming it `envoy.yml`. As it turns out, that doesn't work,
  even when you manually specify the config file. Envoy doesn't recognize the
  `.yml` extension and tries to parse it as JSON! Wtf!

- Now to make a standalone Docker image. I figured I'd try to pack Envoy and the
  server into the same image, because it would be easy to deploy and configure.
  Probably experienced devops engineers would say to have them separate so they
  could scale differently or something, but I feel like this would deployment
  significantly more complicated if I just want to deploy a single server. Maybe
  Kubernetes makes this easy, I dunno.

- At first, I wanted to build the server in a `rust` image, then copy the
  executable into the `envoyproxy/envoy` image. When I tried this, the server
  wouldn't run in the Envoy image because of it not having the right version of
  glibc. I figured I had the choice of either installing Rust into the Envoy
  image so I could build a compatible executable, or installing Envoy into a a
  different image that was capable of running the executable. I went with the
  second, because I feel like my Rust server, the point of the whole exercise,
  shouldn't be beholden to environment oddities on the Envoy image.

- So, I decided to use a `debian/buster-slim` image, copy the Rust executable
  into it, and install Envoy into it via the [somewhat lengthy set of
  commands](https://www.envoyproxy.io/docs/envoy/latest/start/install#install-envoy-on-debian-gnu-linux)
  from their Getting Started guide.

- Because I'm dumb, I had trouble getting the Docker image to run two programs
  at once (Envoy and the gRPC server). At first I tried

  ```dockerfile
  RUN envoy -c /etc/envoy/envoy.yaml &
  CMD ["grpc-web-hello"]
  ```

  then

  ```dockerfile
  RUN nohup envoy -c /etc/envoy/envoy.yaml &
  CMD ["grpc-web-hello"]
  ```

  but neither of these actually ended up with the Envoy server running on
  container startup, plus I learned I don't actually understand what nohup does.
  Eventually I just went with

  ```dockerfile
  CMD envoy -c /etc/envoy/envoy.yaml & grpc-web-hello
  ```

  which does the job. Now everything works! Hooray!

- The `debian:buster-slim` base image is 69 MB. My Rust gRPC server executable
  is only 7 MB (!!). But adding Envoy to the image adds 200 MB. This puts the
  total image size at 276 MB, the vast majority of which is Envoy and its
  dependencies. Hope all that space helps it do its job well!

## Conclusion

Was it worth it? The alternative to gRPC-Web would be to implement the
subscriptions with WebSockets instead. What are the pros and cons?

### gRPC-Web advantages

- Generated stubs. Kind of significant because implementing a WebSocket
  interface and client from scratch is hard work.
- Can use the same interfaces from the frontend as in server-to-server
  communication.

### gRPC-Web disadvantages

- Huge frontend bundle size increase (230 KB parsed, 45 KB gzipped).
- Need to deal with an Envoy reverse-proxy server on the backend. You get this
  for "free" if you were already using Envoy for something, but if not then it
  adds a fair bit of complexity to deployment.
- Unclear if it provides any benefits in speed or bandwidth over JSON, due to
  using base-64 text rather than binary.
- As of today (August 8), the latest version of protoc is completely broken for
  JavaScript.

The drawbacks, especially the first two, are terrible. I can't really recommend
gRPC-Web for serious use in good concience. And yet, in spite of all that I'm
still tempted to keep trying it purely to avoid writing WebSocket communications
by hand.
