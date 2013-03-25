# onTV by udoprog

_Description_: ontv helps you keep track of what you are watching on tv right
now.
Episode information is fetched from the public database thetvdb.com.

## Project Setup

1. Install dependencies listed in requirements.txt
2. Invoke ontv with ./bin/ontv

## Workflow

1. Add the series you are interested in, use __ontv search__ to find them, and
  __ontv add SERIES__ to cache their information locally.
2. Synchronize all information about the tv series you are tracking using
  __ontv sync__.
3. Mark which episodes you have already seen using
  __ontv mark SERIES SEASON [EPISODE]__, whole seasons can be marked.
4. Finally, find out what to see next, and when using __ontv next__.
