import textwrap
import datetime
from dateutil import relativedelta

from .utils import has_aired_filter


def short_episode(episode):
    return u"S{0:02}E{1:02} '{2}'".format(
        episode['season_number'],
        episode['episode_number'],
        episode['episode_name'])


def _build_readable_date(years, months, days):
    show_days = True
    show_weeks = True
    show_months = True

    if years > 1:
        show_days = False
        show_weeks = False
        yield "{0} years".format(years)

        if years >= 10:
            show_months = False

    if years == 1:
        show_days = False
        show_weeks = False
        yield "1 year"

    if show_months:
        if months > 1:
            show_days = False
            yield "{0} months".format(months)

        if months == 1:
            yield "1 month"

    if show_weeks:
        if days > 14:
            yield "{0} weeks".format(days / 7)
            return

        if days > 7:
            yield "1 week"
            days -= 7

    if show_days:
        if days > 1:
            yield "{0} days".format(days)
            return

        yield "1 day"


def readable_timedelta(now, then, next_day, suffix="", prefix=""):
    delta = relativedelta.relativedelta(now, then)

    years = abs(delta.years)
    months = abs(delta.months)
    days = abs(delta.days)

    if years == 0 and months == 0 and days == 0:
        return "today"

    if years == 0 and months == 0 and days == 1:
        return next_day

    g = _build_readable_date(years, months, days)
    return prefix + ", ".join(g) + suffix


def floor_datetime(dt):
    return datetime.datetime(
        year=dt.year,
        month=dt.month,
        day=dt.day,
    )


def format_airdate(aired, now=None):
    if aired is None:
        return "n/a"

    try:
        then = datetime.datetime.strptime(aired, "%Y-%m-%d")
    except:
        return "<invalid %Y-%m-%d>"

    if now is None:
        now = datetime.datetime.now()

    now = floor_datetime(now)

    if then <= now:
        return readable_timedelta(
            now, then, "yesterday", suffix=" ago")

    return readable_timedelta(
        now, then, "tomorrow", prefix="in ")


def format_days(days):
    if days == 1:
        return "1 day"

    return "{0} days".format(days)


def format_wrapped(out, text, indent=u""):
    wrapper = textwrap.TextWrapper()
    wrapper.initial_indent = indent
    wrapper.subsequent_indent = indent

    for line in wrapper.wrap(text):
        out(line)


def format_episode(
    out, term, series_dao, episode,
    short_version=True,
    indent=u"",
):
    color = term.white

    if series_dao.is_episode_watched(episode):
        color = term.bold_blue

    if short_version:
        out(color(
            u"{0}{1:02} '{2}' {3} ({4})".format(
                indent, episode['episode_number'],
                episode['episode_name'],
                format_airdate(episode['first_aired']),
                episode['first_aired'])))
    else:
        out(color(
            u"{0}{1:02} {2}".format(
                indent, episode['episode_number'],
                episode['episode_name'])))

    if short_version:
        return

    out(term.cyan(u"{0}Air date: {1} ({2})".format(
        indent + u"  ",
        format_airdate(episode['first_aired']),
        episode['first_aired'])))

    if episode['overview']:
        format_wrapped(out, episode['overview'], indent=indent + u"  ")

    if 'guest_stars' in episode:
        out(term.cyan(u"{0}Guest stars:".format(indent + u"  ")))
        format_wrapped(out, format_compact_list(episode['guest_stars']),
                       indent=indent + u"    ")


def format_season(
    out, term, series_dao,
    series,
    season_number,
    episodes=None,
    indent=u""
):
    now = datetime.datetime.now()

    has_aired = has_aired_filter(now)

    episodes_legend = format_episodes_count_legend(term)

    all_episodes = series_dao.get_season_episodes(series, season_number)

    episodes_count, stats = format_episodes_count(
        term, series_dao, has_aired, all_episodes)

    color = term.white

    seen, aired, all = stats

    if seen == aired or seen == all:
        color = term.bold_green
    elif seen > 0:
        color = term.bold_yellow
    else:
        color = term.bold_red

    out(u"{0}{c}Season {1}{t.normal} ({2}): {3}".format(
        indent, season_number, episodes_legend, episodes_count,
        c=color, t=term))

    if not episodes:
        return

    for episode in episodes:
        format_episode(out, term, series_dao, episode,
                       short_version=False,
                       indent=indent + u"  ")


def format_compact_list(items, item_format=u"{0}"):
    if items is None:
        return u"(empty)"

    return u", ".join(map(item_format.format, items))


def format_series(
    out, term, series,
    seasons=None,
    ignored_seasons=set(),
    series_dao=None,
    indent=u"",
):
    out(term.bold_cyan(u"{0} (id: {1})".format(
        series['series_name'], series['id'])))

    if 'first_aired' in series:
        out(term.cyan(u"Air date: {0} ({1})".format(
            format_airdate(series['first_aired']),
            series['first_aired'])))

    if 'overview' in series:
        if series['overview']:
            format_wrapped(out, series['overview'], indent=u"  ")

    if not seasons:
        return

    if 'actors' in series:
        out(term.cyan(u"{0}Actors:".format(indent)))
        format_wrapped(out, format_compact_list(series['actors']),
                       indent=indent + u"  ")

    out(term.cyan(u"Seasons"))

    for season_number, season_episodes in sorted(seasons.items()):
        if season_number in ignored_seasons:
            continue

        if len(seasons) != 1:
            season_episodes = None

        format_season(
            out, term, series_dao, series, season_number,
            episodes=season_episodes,
            indent=u"  ")


def format_episodes_count_legend(term):
    return (
        u"{t.green}{0}{t.normal}/"
        u"{t.yellow}{1}{t.normal}/"
        u"{t.red}{2}{t.normal}"
    ).format(
        "seen", "aired", "all",
        t=term)


def format_episodes_count(term, series_dao, has_aired, episodes):
    seen_episodes = len(filter(
        series_dao.is_episode_watched, episodes))

    aired_episodes = len(filter(has_aired, episodes))

    all_episodes = len(episodes)

    stats = (seen_episodes, aired_episodes, all_episodes)

    return (
        u"{t.green}{0}{t.normal}/"
        u"{t.yellow}{1}{t.normal}/"
        u"{t.red}{2}{t.normal}"
    ).format(*stats, t=term), stats
