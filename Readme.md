# DNB Crawl

An application for the automatic extraction of financial statements from DNB.

## Requirements

I would recommend using a python distribution system such as [Conda](https://docs.conda.io/en/latest/miniconda.html) to manage dependencies, but if you want to manage your own environment here are the requirements:

- Python 3.9
- Selenium
- pyyaml
- pypdf2

### Driver versions included

While there are some drivers already included with the repository, they may at some point be out of date. As such, the new releases can be found on the respective sites below. If they also at some point become corrupted, just download them again and replace them in the folder called `drivers`

- [Firefox](https://github.com/mozilla/geckodriver/releases): 0.29.0

## Configuration

The application uses a yaml file to determine which accounts are to be processed and which dates are needed. The format goes as follows:

```yaml
- account: "####.##.#####"
  duration:
    - "01/2020"
    - "01/2021"

```

There has to be at least one block like the one above, but there can be as many as you want to extract as long as they are separated by an empty line. The months have to be zero padded on the left with 2 digits (ie. January is '01', but December is only '12'). The year also has to be in 4 digits. The `#`s have to be replaced by the actual account number for the program to work as well.
