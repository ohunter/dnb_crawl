# DNB Crawl

An application for the automatic extraction of financial statements from DNB.

## Requirements

I would recommend using a python distribution system such as [Conda](https://docs.conda.io/en/latest/miniconda.html) to manage dependencies, but if you want to manage your own environment here are the requirements:

- Python 3.9
- Selenium
- pyyaml
- pypdf2

### Driver versions included

While there are some drivers already included with the repository, they may at some point be out of date.
As such, the new releases can be found on the respective sites below.
If they also at some point become corrupted, just download them again and replace them in the folder called `drivers`

- [Firefox](https://github.com/mozilla/geckodriver/releases): 0.32.0

## Configuration

The application uses a yaml file to determine which accounts are to be processed and which dates are needed.
The format goes as follows:

```yaml
ssn: 00000000000              # The user identification number
extractions:                  # Contains all the different accounts to be extracted
  - start: "01/2020"          # The first month
    stop: "01/2021"           # The month after the last one needed
    accounts:                 # All the accounts which should be handled within this duration
    - id: "####.##.#####"     # This is the account number that will be looked for on DNB's pages
      name: "example account" # This will be the name of the file pertaining to this account
```

The `ssn` field corresponds to the number you enter when you first want to log on to DNB.
This line is optional, and if excempt you will have to enter it yourself when prompted to.
In order to add process more than one account for a given period of time, simply add another line like the last one below it.
The indentation is important, so make sure it is the same.
In order to process different ranges for accounts, copy the lines from the `from` to the bottom and fill in the information as needed.
The months have to be zero padded on the left with 2 digits (ie.
January is '01', but December is only '12').
The year also has to be in 4 digits.
The `#`s have to be replaced by the actual account number for the program to work as well.
The `name` field is an optional field which allows the user to specify an alias for the account that will become the file name.
