import yaml
import sys
import pandas as pd

if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("Please provide excel file as second argument")
        sys.exit(0)

    df = pd.read_excel(sys.argv[1])
    with open("config.yaml", "w") as fi:

        yaml.dump({
            "ssn": 00000000000,
            "extractions": [
                {
                    "start": "01/2022",
                    "stop": "01/2023",
                    "accounts": [
                        {
                            "id": x.Account.strip(),
                            "name": x.Filename.strip()
                        } for x in df.itertuples()
                    ],
                }
            ]
        }, fi, sort_keys=False)