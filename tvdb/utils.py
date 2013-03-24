import yaml
import functools


def write_yaml(fp, document):
    """
    Write a document to the specified file descriptor that is formatted in a
    readable way.
    """
    yaml.dump(document, fp, default_flow_style=False)


def read_yaml(fp):
    return yaml.load(fp)


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
        title="Pick a seires (ctrl-d to abort): ")


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


def with_series(finder):
    def __with_series(func):
        @functools.wraps(func)
        def __inner(ns):
            try:
                series = finder(ns, ns.series_id)
            except Exception as e:
                print ns.term.bold_red(str(e))
                return 1

            return func(ns, series)

        return __inner

    return __with_series


def local_series_finder(ns, query_id):
    """
    Find a series by id from the local database.

    :query_id The id of the series to find.
    """
    return series_finder(ns.series.find_series, ns.series.get, query_id)


def api_series_finder(ns, query_id):
    """
    Find a series by querying the remote database through the api.
    """
    if ns.language is None:
        raise Exception("langauge must be configured")

    def get_series(series_id):
        return ns.api.series(series_id, ns.language)

    return series_finder(ns.api.getseries, get_series, query_id)


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
