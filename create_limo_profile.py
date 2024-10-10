"""
This script is used to generate a new api key for the limo profile for the limonade service.
"""

import requests
import random

def main():
    headers = {'Authorization':'Bearer admin'}
    random_id = random.randint(0, 10000000)
    response = requests.post('http://localhost:9000/v1/profiles', json={'name': 'limo', 'email':f'limo{random_id}@dourolabs.com','role':'protocol'}, headers=headers)
    profile_id = response.json()['id']

    response = requests.post('http://localhost:9000/v1/profiles/access_tokens', json={'profile_id':profile_id}, headers=headers)
    access_token = response.json()['token']
    print(access_token)



if __name__ == '__main__':
    main()
