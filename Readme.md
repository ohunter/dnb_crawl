# DNB Crawl

An application for the automatic extraction of financial statements from DNB.

## Requirements

I would recommend using a python distribution system such as [Conda](https://docs.conda.io/en/latest/miniconda.html) to manage dependencies, but if you want to manage your own environment here are the requirements:

- Python 3.9
- Selenium
- pyyaml

### Driver versions included

While there are some drivers already included with the repository, they may at some point be out of date. As such, the new releases can be found on the respective sites below. If they also at some point become corrupted, just download them again and replace them in the folder called `drivers`

- [Firefox](https://github.com/mozilla/geckodriver/releases): 0.29.0
- [Chrome](https://sites.google.com/a/chromium.org/chromedriver/downloads): 89.0.4389.23
