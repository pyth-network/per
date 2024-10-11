"""
This script is used to generate a new api key for the limo profile for the limonade service.
"""

import requests
import random

def main():
    headers = {'Authorization':'Bearer admin'}
    limo_email='limo@dourolabs.com'
    response = requests.post('http://localhost:9000/v1/profiles', json={'name': 'limo', 'email':limo_email,'role':'protocol'}, headers=headers)
    if response.status_code == 400:
        response = requests.get('http://localhost:9000/v1/profiles', headers=headers, params={'email':limo_email})
    profile_id = response.json()['id']

    response = requests.post('http://localhost:9000/v1/profiles/access_tokens', json={'profile_id':profile_id}, headers=headers)
    access_token = response.json()['token']
    print(access_token)



if __name__ == '__main__':
    main()
