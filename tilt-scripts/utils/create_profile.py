"""
This script is used to generate a new api key and profile.
"""

import argparse
import logging

import requests

logger = logging.getLogger(__name__)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("-v", "--verbose", action="count", default=0)
    parser.add_argument(
        "--name",
        type=str,
        required=True,
        help="Name of the profile",
    )
    parser.add_argument(
        "--email",
        type=str,
        required=True,
        help="Email of the profile",
    )
    parser.add_argument(
        "--role",
        type=str,
        required=True,
        help="Role of the profile",
    )
    return parser.parse_args()


def main(name: str, email: str, role: str):
    headers = {"Authorization": "Bearer admin"}
    response = requests.post(
        "http://localhost:9000/v1/profiles",
        json={"name": name, "email": email, "role": role},
        headers=headers,
    )
    if response.status_code == 400:
        response = requests.get(
            "http://localhost:9000/v1/profiles",
            headers=headers,
            params={"email": email},
        )
    profile_id = response.json()["id"]

    response = requests.post(
        "http://localhost:9000/v1/profiles/access_tokens",
        json={"profile_id": profile_id},
        headers=headers,
    )
    access_token = response.json()["token"]
    print(access_token)


if __name__ == "__main__":
    args = parse_args()
    main(args.name, args.email, args.role)
