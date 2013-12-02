# onTV

ontv helps you keep track of what you are watching on tv right
now.
Episode information is fetched from the public database thetvdb.com.

## Installation

  pip install ontv

## Project Setup

1. Install dependencies; __pip install -r requirements.txt__
2. Invoke ontv with ./bin/ontv

## Workflow

First you need to register on thetvdb.com and get an api key, follow the
instructions [http://thetvdb.com/?tab=register](on thetvdb.com)

1. Synchronize all information about the tv series you are tracking using
  __ontv sync__. _This is required the first time you use ontv._
2. Add the series you are interested in, use __ontv search__ to find them, and
  __ontv add &lt;series&gt;__ to cache their information locally.
3. Mark which episodes you have already seen using
  __ontv mark &lt;series&gt; ([&lt;season&gt; [&lt;episode&gt;]]|--next)__,
  whole seasons can be marked in one sweep.
4. Finally, find out what to see next, and when using __ontv next__.

__ontv list__ gives you a comprehensive view of what you are watching right
now.
