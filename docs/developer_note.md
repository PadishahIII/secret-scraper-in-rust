Todo:
- [x] Implement CLI: clap
- [x] Implement logger via tracing
- [x] Implement Crawler
    - [x] implement options
    - [x] write tests
- [x] Implement rate limiter
- [x] Implement output formatter, tested
- [x] Implement lib interface: builder...
- [x] Add unit tests
- [x] Add integration tests
- [x] Add examples
- [x] Resolve lint warnings
- [x] Update doc
- [ ] Implement graceful shutdown
- [ ] Add clap CLI tests
- [ ] Test performance and compare with the python version; use `time` tool
    - `gtime --verbose ./target/release/secret_scraper -u https://www.baidu.com --detail --allow-domains www.baidu.com  -o 1.csv `
- [ ] Publish


## Performance test
Python version:
```
gtime uv run src/secretscraper/cmdline.py -u https://www.baidu.com --allow-domains www.baidu.com -o 1.csv --detail
```
```
 Command being timed: "uv run src/secretscraper/cmdline.py -u https://www.baidu.com --allow-domains www.baidu.com -o 1.csv --detail"
	User time (seconds): 5.42
	System time (seconds): 0.23
	Percent of CPU this job got: 8%
	Elapsed (wall clock) time (h:mm:ss or m:ss): 1:10.17
	Average shared text size (kbytes): 0
	Average unshared data size (kbytes): 0
	Average stack size (kbytes): 0
	Average total size (kbytes): 0
	Maximum resident set size (kbytes): 121328
	Average resident set size (kbytes): 0
	Major (requiring I/O) page faults: 662
	Minor (reclaiming a frame) page faults: 19771
	Voluntary context switches: 2370
	Involuntary context switches: 9459
	Swaps: 0
	File system inputs: 0
	File system outputs: 0
	Socket messages sent: 382
	Socket messages received: 468
	Signals delivered: 1
	Page size (bytes): 16384
	Exit status: 0
```

Rust version:
```
gtime --verbose ./target/release/secret_scraper -u https://www.baidu.com --detail --allow-domains www.baidu.com  -o 1.csv 
```

