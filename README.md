This is a small single file Rust program that I used ChatGPT4 to make the entirety of.
I do not know how to program in Rust or really at all with regards to normal conventions and best practices.
I just have kind of spotty internet and wanted a free tool to help identify when blips happened and for how long.

How it works:
When you start monitoring, the program will send a 32 byte ping to Google every second. The round trip time is noted and displayed.
When a response time >175 ms is detected, a textfile log containing the last 100 seconds of pings is created and saved to the working 
directory. No more than one log is created every minute. You can force a log creation using the `Log Data` button.
