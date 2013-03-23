import yaml


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


def find_series_external(term, fetch_series, series_id):
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


def find_series(term, fetch_series, series_id):
    try:
        return int(series_id)
    except:
        return find_series_external(
            term, fetch_series, series_id)


def group_episodes(episodes):
    seasons = dict()

    for episode in episodes:
        season_number = episode.get("season_number", 0)
        seasons.setdefault(season_number, []).append(episode)

    return seasons
