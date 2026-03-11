Output demo.gif

Set Shell "bash"
Set FontSize 13
Set Width 1200
Set Height 680
Set Padding 20
Set TypingSpeed 40ms

# Show help
Type "httpress --help"
Enter
Sleep 4s

Ctrl+L
Sleep 500ms

# Scene 2: GET with duration, concurrency, rate, timeout
Type "httpress https://httpbin.org/get -c 20 -d 5s -r 100 -t 10"
Enter
Sleep 8s

Ctrl+L
Sleep 500ms

# Scene 3: POST with method, headers, body, fixed request count
Type `httpress https://httpbin.org/post -m post -c 5 -n 50 -H "Content-Type: application/json" -H "X-Demo: httpress" -b '{"hello":"world"}'`
Enter
Sleep 12s
