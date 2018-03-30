const start = Date.now()

function write(data) {
  process.stdout.write(
    typeof data === 'string' ? data : JSON.stringify(Object.assign({
      start,
      nested: {
        thing: 'something',
        things: [
          { foo: 'bar', baz: 'buz' }
        ]
      }
    }, data))
  )
}

const timer = setInterval(() => {
  write({ now: Date.now() })
}, 100)

setTimeout(() => {
  clearInterval(timer)
  write({ now: Date.now() })
}, 10000)
