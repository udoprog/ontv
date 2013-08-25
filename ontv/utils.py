import yaml
import functools
import datetime

DATETIME_FORMAT = "%Y-%m-%d"


def write_yaml(fp, document):
    """
    Write a document to the specified file descriptor that is formatted in a
    readable way.
    """
    yaml.dump(document, fp, default_flow_style=False)


def read_yaml(fp):
    return yaml.load(fp)


def format_datetime(d):
    return d.strftime(DATETIME_FORMAT)


def parse_datetime(s):
    return datetime.datetime.strptime(s, DATETIME_FORMAT)


def pick_one(alternatives, title="Pick one: ", indent="  "):
    while True:
        for i, (alternative, _) in enumerate(alternatives):
            print u"{0}{1}) {2}".format(indent, i, alternative)

        try:
            user_input = raw_input(title)
        except EOFError:
            print "No input"
            return None

        try:
            user_input = int(user_input)
        except:
            print "Invalid input '{0}'".format(user_input)
            continue

        if user_input < 0:
            return None

        try:
            _, data = alternatives[user_input]
        except IndexError:
            print "Invalid index {0}, try one between 0-{1}".format(
                user_input, len(alternatives) - 1)
            continue

        return data


def find_series_external(fetch_series, series_id):
    results = fetch_series(series_id)

    if len(results) <= 0:
        return None

    if len(results) == 1:
        return results[0]['id']

    alternatives = [
        (u"{0[series_name]} ({0[id]})".format(s), s['id']) for s in results]

    return pick_one(
        alternatives,
        title="Pick a series (ctrl-d to abort): ")


def find_series(fetch_series, series_id):
    try:
        return int(series_id)
    except:
        return find_series_external(fetch_series, series_id)


def group_episodes(episodes):
    seasons = dict()

    for episode in episodes:
        season_number = episode.get("season_number", 0)
        seasons.setdefault(season_number, []).append(episode)

    return seasons


def _episode_key(episode):
    """
    Returns key used to sort episodes based on season and episode number.
    """
    return (episode['season_number'], episode['episode_number'])


def sorted_episodes(episodes):
    return sorted(episodes, key=_episode_key)


def series_finder(fetch_series, get_series, query_id):
    """
    Find series from the local database.
    """
    result_id = find_series(fetch_series, query_id)

    if result_id is None:
        raise Exception(u"no such series: {0}".format(query_id))

    series = get_series(result_id)

    if series is None:
        raise Exception(u"no such series (id): {0}".format(result_id))

    return series


def numeric_ranges(s):
    """
    Parse numeric ranges separated by comma.

    A numeric range can either be a single number, or a range separated by a
    dash.

    Overlaps are ignored since we are storing the result in a set.

    Example: 1,2,10-15,22 -> set([1,2,10,11,12,13,14,15,22])
    """
    numbers = set()

    parts = s.split(',')

    for p in parts:
        if '-' in p:
            left, right = p.split('-')
            numbers.update(range(int(left), int(right) + 1))
        else:
            numbers.add(int(p))

    return numbers


def numeric_range(s):
    if '-' not in s:
        raise Exception(
            ("not a valid range, expected '<begin>-<end>' but "
             "was '{0}'").format(s))

    begin, end = s.split("-", 1)

    try:
        return int(begin), int(end)
    except:
        raise Exception(
            "range does not contain numeric values '{0}'".format(s))


def find_next_episode(episodes, is_watched, ignored_seasons=set([0])):
    """
    Find the next episode that is not watched.

    :episodes: A list of all the episodes.
    :is_watched: A function returning true if the episode has been watched.
    :ignored_seasons: Seasons to ignore when checking if watched or not.

    Returns None if none can be found.
    """

    for episode in sorted_episodes(episodes):
        if episode['season_number'] in ignored_seasons:
            continue

        if is_watched(episode):
            continue

        if not episode['first_aired']:
            continue

        airdate = parse_datetime(episode['first_aired'])
        return episode, airdate

    return None


def has_aired_filter(now):
    """
    Utility function to create an episode filter for if an episode has been
    aired.
    """

    def __has_aired(e):
        if not e['first_aired']:
            return False
        return now >= parse_datetime(e['first_aired'])

    return __has_aired


# the following functions require a namespace since separating them is not
# worth the effort right now.


def local_series_finder(ns):
    """
    Find a series by id from the local database.

    :query_id The id of the series to find.
    """

    if ns.series_query is None:
        return None

    return series_finder(ns.series.find_series, ns.series.get, ns.series_query)


def local_episodes_finder(ns, series):
    """
    Find local episodes.

    Expectes series to be provided as an argument.
    """
    episodes = ns.series.get_episodes(series)

    if episodes is None:
        raise Exception("No episodes synced for series: {0}".format(
            series['series_name']))

    return list(find_episodes(ns, episodes))


def api_series_finder(ns):
    """
    Find a series by querying the remote database through the api.
    """
    if ns.language is None:
        raise Exception("language must be configured")

    def get_series(series_id):
        return ns.api.series(series_id, ns.language)

    return series_finder(ns.api.getseries, get_series, ns.series_query)


def with_resource(finder):
    def __with_resource(func):
        @functools.wraps(func)
        def __inner(ns, *args, **kw):
            try:
                resource = finder(ns, *args)
            except Exception as e:
                print ns.t.bold_red(str(e))
                return 1

            args = list(args) + [resource]
            return func(ns, *args, **kw)

        return __inner

    return __with_resource


def find_episodes(ns, episodes):
    if ns.next:
        result = find_next_episode(
            episodes, ns.series.is_episode_watched,
            ignored_seasons=ns.ignored_seasons)

        if result is None:
            print ns.t.bold_red(u"no episode is next")
            return

        next_episode, next_airdate = result
        yield next_episode
        return

    for episode in episodes:
        if ns.seasons is not None:
            if episode['season_number'] not in ns.seasons:
                continue

        if ns.episodes is not None:
            if episode['episode_number'] not in ns.episodes:
                continue

        yield episode

    return
