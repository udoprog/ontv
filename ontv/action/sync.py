from ..utils import write_yaml


def action(ns):
    print ns.t.bold_magenta(u"Synchronizing local database")
    print u""

    if not ns.api.is_authenticated():
        print ns.t.bold_red(
            "Cannot fully synchronize, api is not authenticated with a key")
        print ns.t.bold_red(
            "Set a value to the 'api_key' option in {0}".format(
                ns.config_path))
        return 0

    if not ns.languages:
        with open(ns.languages_path, 'w') as fp:
            write_yaml(fp, ns.api.languages())

    if not ns.mirrors:
        print ns.t.bold_green("Downloading mirrors to {0}".format(
            ns.mirrors_path))

        with open(ns.mirrors_path, 'w') as fp:
            write_yaml(fp, ns.api.mirrors())

    for series in ns.series.list_series():
        print ns.t.bold_cyan(
            u"syncing: {0}".format(series['series_name']))

        series, episodes = ns.api.series_all(
            series['id'], ns.language)

        ns.series.set_episodes(series, episodes)

    return 0


def setup(parser):
    parser.set_defaults(action=action)
